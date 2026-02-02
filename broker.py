from __future__ import annotations
import os, time
from dataclasses import dataclass, field
from typing import Dict, Optional, Literal, Any
from .config import CONFIG, is_armed
from .utils import log, STATE, update_drawdown
from .microstructure import get_market_rules, normalize_order

Side = Literal["LONG","SHORT","FLAT"]

try:
    import ccxt.async_support as ccxt_async  # type: ignore
except Exception:
    ccxt_async = None  # type: ignore

@dataclass
class Position:
    qty: float = 0.0
    avg: float = 0.0
    realized: float = 0.0

@dataclass
class Portfolio:
    cash: float
    equity: float
    positions: Dict[str, Position] = field(default_factory=dict)

    def pos(self, sym: str) -> Position:
        if sym not in self.positions:
            self.positions[sym] = Position()
        return self.positions[sym]

class BaseBroker:
    def __init__(self):
        start = float(CONFIG.get("STARTING_EQUITY", 0.0))
        self.port = Portfolio(cash=start, equity=start)
        self.start_equity = start
        self.last_trade_ts: Dict[str, float] = {}

    def _cooldown_ok(self, sym: str) -> bool:
        cd = float(CONFIG.get("COOLDOWN_SEC", 0))
        if cd <= 0: return True
        return (time.time() - self.last_trade_ts.get(sym, 0.0)) >= cd

    def _risk_ok(self, units: float, price: float) -> bool:
        eq = float(STATE.get("equity", self.port.equity))
        lev = float(CONFIG.get("LEVERAGE", 1.0))
        if lev > float(CONFIG.get("MAX_LEVERAGE_CAP", 3.0)):
            return False
        notional = abs(units)*price*lev
        if notional > float(CONFIG.get("MAX_EXPOSURE_PCT", 0.05))*eq:
            return False
        if float(STATE.get("drawdown", 0.0)) > float(CONFIG.get("DD_PAUSE", 0.25)):
            return False
        return True

    async def refresh_equity(self, prices: Dict[str,float]) -> None:
        eq = self.port.cash
        lev = float(CONFIG.get("LEVERAGE", 1.0))
        for sym, p in prices.items():
            pos = self.port.positions.get(sym)
            if not pos or pos.qty == 0: continue
            eq += pos.qty * (p - pos.avg) * lev
        self.port.equity = float(eq)
        STATE["equity"] = float(eq)
        update_drawdown(self.start_equity)

class PaperBroker(BaseBroker):
    def __init__(self, slippage_bps: float = 8.0):
        super().__init__()
        self.slip = float(slippage_bps)/10000.0

    def _fill(self, side: Side, p: float) -> float:
        return p*(1+self.slip) if side=="LONG" else p*(1-self.slip)

    async def execute(self, sym: str, side: Side, units: float, price: float, venue: str = "PAPER", exec_style: str = "TAKER", meta: dict | None = None) -> dict:
        if units <= 0:
            return {}
        if not self._cooldown_ok(sym):
            return {}
        if price > 0 and not self._risk_ok(units, price):
            return {}

        if not self._armed():
            log.warning(f"[BTCC:DRY] would {side} {sym} units={units:.6f} meta={meta or {}}")
            return {}

        meta = meta or {}
        client_oid = str(meta.get("client_oid") or "")
        post_only = bool(meta.get("post_only", bool(CONFIG.get("MAKER_POST_ONLY", True))))

        try:
            if exec_style == "MAKER" and price > 0:
                params = {}
                if post_only:
                    params.update({"postOnly": True})
                if client_oid:
                    params.update({"clientOrderId": client_oid})
                if side == "LONG":
                    resp = await self.ex.create_limit_buy_order(sym, units, price, params)
                elif side == "SHORT":
                    resp = await self.ex.create_limit_sell_order(sym, units, price, params)
                else:
                    return {}
                self.last_trade_ts[sym] = time.time()
                log.info(f"[BTCC] LIMIT {side} {sym} units={units:.4f} px={price:.4f} post_only={post_only}")
                return resp or {}

            if side == "LONG":
                resp = await self.ex.create_market_buy_order(sym, units)
            elif side == "SHORT":
                resp = await self.ex.create_market_sell_order(sym, units)
            else:
                return {}
            self.last_trade_ts[sym] = time.time()
            log.info(f"[BTCC] MKT {side} {sym} units={units:.4f}")
            return resp or {}
        except Exception as e:
            log.error(f"[BTCC] order failed: {e}")
            return {}

    async def fetch_positions(self, symbols: list[str]) -> Dict[tuple[str, str], float]:
        """Best-effort positions snapshot for BTCC perps.

        ccxt coverage varies by exchange and by version; we attempt multiple methods.
        Returns mapping {(sym, 'BTCC_PERP'): qty} where qty is +long / -short in units.
        """
        out: Dict[tuple[str, str], float] = {}
        if ccxt_async is None:
            return out
        try:
            # Prefer fetch_positions if supported
            if hasattr(self.ex, "fetch_positions"):
                pos = await self.ex.fetch_positions(symbols)
                for p in pos or []:
                    s = str(p.get("symbol") or "")
                    if s not in symbols:
                        continue
                    amt = float(p.get("contracts") or p.get("positionAmt") or p.get("amount") or 0.0)
                    side = str(p.get("side") or "").lower()
                    if side == "short":
                        amt = -abs(amt)
                    out[(s, "BTCC_PERP")] = float(amt)
                return out
        except Exception:
            pass
        # Fallback: cannot reliably obtain perps positions
        return out


class MultiVenueSpotBroker(BaseBroker):
    """Spot broker that can route to Coinbase, Kraken, Binance.US (or paper).

    In paper/shadow modes, orders fill through PaperBroker.
    In live mode, `create_market_*_order` is used (simple and safe). Execution sophistication
    (maker vs taker, slicing) is intentionally handled at a higher layer.
    """

    def __init__(self, paper: PaperBroker):
        super().__init__()
        self.paper = paper
        self.dry_run = bool(CONFIG.get("DRY_RUN", True))

        self.venue_ids = {
            "COINBASE": str(CONFIG.get("COINBASE_EXCHANGE_ID", "coinbase")),
            "KRAKEN": str(CONFIG.get("KRAKEN_EXCHANGE_ID", "kraken")),
            "BINANCEUS": str(CONFIG.get("BINANCEUS_EXCHANGE_ID", "binanceus")),
            "BTCC_SPOT": str(CONFIG.get("BTCC_EXCHANGE_ID", "btcc")),
        }

        self._ex_cache: Dict[str, Any] = {}

    def _armed(self) -> bool:
        return is_armed()

    def _get_keys(self, venue: str) -> Dict[str, str]:
        # Prefer config; fall back to env; interactive prompting is handled in load_env_keys/config.
        if venue == "COINBASE":
            return {"apiKey": CONFIG.get("COINBASE_API_KEY", ""), "secret": CONFIG.get("COINBASE_API_SECRET", "")}
        if venue == "KRAKEN":
            return {"apiKey": CONFIG.get("KRAKEN_API_KEY", ""), "secret": CONFIG.get("KRAKEN_API_SECRET", "")}
        if venue == "BINANCEUS":
            return {"apiKey": CONFIG.get("BINANCEUS_API_KEY", ""), "secret": CONFIG.get("BINANCEUS_API_SECRET", "")}
        if venue == "BTCC_SPOT":
            return {"apiKey": CONFIG.get("BTCC_API_KEY", ""), "secret": CONFIG.get("BTCC_API_SECRET", "")}
        return {"apiKey": "", "secret": ""}

    def _get_ex(self, venue: str):
        if ccxt_async is None:
            return None
        if venue in self._ex_cache:
            return self._ex_cache[venue]
        ex_id = self.venue_ids.get(venue)
        if not ex_id:
            return None
        ex = getattr(ccxt_async, ex_id)()
        keys = self._get_keys(venue)
        ex.apiKey = keys.get("apiKey", "")
        ex.secret = keys.get("secret", "")
        self._ex_cache[venue] = ex
        return ex

    async def close(self):
        for ex in self._ex_cache.values():
            try:
                await ex.close()
            except Exception:
                pass
        self._ex_cache.clear()

    async def fetch_positions(self, symbols: list[str], venue: str) -> Dict[tuple[str, str], float]:
        """Best-effort spot positions snapshot for a given venue.

        Returns mapping {(sym, venue): qty} where qty is base-asset units.
        """
        out: Dict[tuple[str, str], float] = {}
        ex = self._get_ex(venue)
        if ex is None:
            return out
        # If keys missing, ccxt may still allow public balance calls on some venues, but we treat it as best-effort.
        try:
            bal = await ex.fetch_balance()
            total = (bal or {}).get("total") or {}
            for sym in symbols:
                base = sym.split("/")[0]
                qty = float(total.get(base) or 0.0)
                if abs(qty) > 0:
                    out[(sym, venue)] = qty
        except Exception:
            return out
        return out

    async def execute(self, sym: str, side: Side, units: float, price: float, venue: str, exec_style: str = "TAKER", meta: dict | None = None) -> None:
        if units <= 0 or price <= 0:
            return
        if not self._cooldown_ok(sym):
            return
        if not self._risk_ok(units, price):
            return

        # In paper/shadow or not armed -> paper fill
        mode = str(CONFIG.get("MODE", "shadow")).lower()
        if mode in ("paper", "shadow") or (not self._armed()):
            return await self.paper.execute(sym, side, units, price, venue=venue or "PAPER", exec_style=exec_style, meta=meta)

        ex = self._get_ex(venue)
        if ex is None:
            log.error(f"[SPOT] venue {venue} unavailable; falling back to paper")
            return await self.paper.execute(sym, side, units, price, venue=venue or "PAPER", exec_style=exec_style, meta=meta)

        meta = meta or {}

        # --- Institutional microstructure normalization (tick sizes, min notional) ---
        try:
            if not getattr(ex, "markets", None):
                await ex.load_markets()
            m = ex.market(sym) if hasattr(ex, "market") else (ex.markets or {}).get(sym)
            if isinstance(m, dict):
                rules = get_market_rules(m)
                min_notional_map = CONFIG.get("MIN_NOTIONAL_BY_VENUE", {}) or {}
                min_notional = min_notional_map.get(str(venue).upper())
                npx, nqty, ok, reason = normalize_order(price=price, qty=units, side=side, rules=rules, min_notional_override=min_notional)
                price, units = float(npx), float(nqty)
                if not ok:
                    log.warning(f"[SPOT:{venue}] microstructure reject {sym} reason={reason} px={price} qty={units}")
                    return
        except Exception:
            # Best-effort; never hard-fail execution if markets cannot be loaded.
            pass
        client_oid = str(meta.get("client_oid") or "")
        post_only = bool(meta.get("post_only", bool(CONFIG.get("MAKER_POST_ONLY", True))))

        # Exchange-specific post-only parameter handling.
        # ccxt is mostly consistent, but some venues expect different param keys.
        po_keys = CONFIG.get("POST_ONLY_PARAM_KEYS", {}) or {}
        po_key = str(po_keys.get(str(venue).upper(), po_keys.get("DEFAULT", "postOnly")))

        try:
            # Institutional OMS primitive: limit orders + optional post-only.
            if exec_style == "MAKER":
                params = {}
                if post_only:
                    # Set only the configured key to avoid venue-specific errors.
                    params.update({po_key: True})
                if client_oid:
                    params.update({"clientOrderId": client_oid})

                if side == "LONG":
                    resp = await ex.create_limit_buy_order(sym, units, price, params)
                elif side == "SHORT":
                    resp = await ex.create_limit_sell_order(sym, units, price, params)
                else:
                    return {}
                self.last_trade_ts[sym] = time.time()
                log.info(f"[SPOT:{venue}] LIMIT {side} {sym} units={units:.6f} px={price:.4f} post_only={post_only}")
                return resp or {}

            # Default: market order
            if side == "LONG":
                resp = await ex.create_market_buy_order(sym, units)
            elif side == "SHORT":
                resp = await ex.create_market_sell_order(sym, units)
            else:
                return {}
            self.last_trade_ts[sym] = time.time()
            log.info(f"[SPOT:{venue}] MKT {side} {sym} units={units:.6f}")
            return resp or {}
        except Exception as e:
            log.error(f"[SPOT:{venue}] order failed: {e}")
            # fail closed -> no retry loop here

    async def fetch_orderbook(self, sym: str, venue: str, limit: int = 50) -> dict:
        ex = self._get_ex(venue)
        if ex is None:
            return {}
        try:
            return await ex.fetch_order_book(sym, limit=limit)
        except Exception:
            return {}

    async def fetch_open_orders(self, sym: str, venue: str) -> list[dict]:
        ex = self._get_ex(venue)
        if ex is None:
            return []
        try:
            if hasattr(ex, "fetch_open_orders"):
                return await ex.fetch_open_orders(sym)
        except Exception:
            return []
        return []

    async def cancel_order(self, order_id: str, sym: str, venue: str) -> bool:
        ex = self._get_ex(venue)
        if ex is None:
            return False
        try:
            if hasattr(ex, "cancel_order"):
                await ex.cancel_order(order_id, sym)
                return True
        except Exception:
            return False
        return False



    async def fetch_order(self, order_id: str, sym: str, venue: str) -> dict:
        'Fetch an order by exchange order id (best-effort).'
        ex = self._get_ex(venue)
        if ex is None:
            return {}
        try:
            if hasattr(ex, 'fetch_order'):
                return await ex.fetch_order(order_id, sym)
        except Exception:
            return {}
        return {}
    async def fetch_my_trades(self, sym: str, venue: str, since_ms: int | None = None, limit: int = 200) -> list[dict]:
        ex = self._get_ex(venue)
        if ex is None:
            return []
        try:
            if hasattr(ex, "fetch_my_trades"):
                return await ex.fetch_my_trades(sym, since=since_ms, limit=limit)
        except Exception:
            return []
        return []



class BTCCPerpsBroker(BaseBroker):
    """BTCC perpetuals broker (live).

    Institutional-grade notes:
    - Perps APIs vary across ccxt versions.
    - This broker is best-effort and FAILS CLOSED: if required perps risk data
      is unavailable and PERP_REQUIRE_RISK_DATA is true, the risk layer will
      disable perps orders.
    """

    def __init__(self):
        super().__init__()
        if ccxt_async is None:
            raise RuntimeError("ccxt.async_support not available")
        ex_id = str(CONFIG.get("BTCC_EXCHANGE_ID", "btcc"))
        self.ex = getattr(ccxt_async, ex_id)()
        self.ex.apiKey = str(CONFIG.get("BTCC_API_KEY", "") or "")
        self.ex.secret = str(CONFIG.get("BTCC_API_SECRET", "") or "")
        self.dry_run = bool(CONFIG.get("DRY_RUN", True))

        # Prefer swap markets when supported.
        try:
            self.ex.options = getattr(self.ex, "options", {}) or {}
            self.ex.options.update({"defaultType": "swap"})
        except Exception:
            pass

    def _armed(self) -> bool:
        return is_armed()

    async def close(self):
        try:
            await self.ex.close()
        except Exception:
            pass

    async def execute(self, sym: str, side: Side, units: float, price: float, venue: str = "BTCC_PERP", exec_style: str = "TAKER", meta: dict | None = None) -> dict:
        if units <= 0:
            return {}
        meta = meta or {}
        client_oid = str(meta.get("client_oid") or "")
        post_only = bool(meta.get("post_only", bool(CONFIG.get("MAKER_POST_ONLY", True))))

        if not self._armed():
            log.warning(f"[BTCC_PERP:DRY] would {side} {sym} units={units:.6f} style={exec_style} meta={meta}")
            return {}

        try:
            if exec_style == "MAKER" and price > 0:
                params = {}
                if post_only:
                    params.update({"postOnly": True})
                if client_oid:
                    params.update({"clientOrderId": client_oid})
                if side == "LONG":
                    resp = await self.ex.create_limit_buy_order(sym, units, price, params)
                elif side == "SHORT":
                    resp = await self.ex.create_limit_sell_order(sym, units, price, params)
                else:
                    return {}
                self.last_trade_ts[sym] = time.time()
                log.info(f"[BTCC_PERP] LIMIT {side} {sym} units={units:.4f} px={price:.4f} post_only={post_only}")
                return resp or {}

            if side == "LONG":
                resp = await self.ex.create_market_buy_order(sym, units)
            elif side == "SHORT":
                resp = await self.ex.create_market_sell_order(sym, units)
            else:
                return {}
            self.last_trade_ts[sym] = time.time()
            log.info(f"[BTCC_PERP] MKT {side} {sym} units={units:.4f}")
            return resp or {}
        except Exception as e:
            log.error(f"[BTCC_PERP] order failed: {e}")
            return {}

    async def fetch_positions(self, symbols: list[str]) -> Dict[tuple[str, str], float]:
        """Return {(sym,'BTCC_PERP'): qty} best-effort."""
        out: Dict[tuple[str, str], float] = {}
        try:
            if hasattr(self.ex, "fetch_positions"):
                pos = await self.ex.fetch_positions(symbols)
                for p in pos or []:
                    s = str(p.get("symbol") or "")
                    if s not in symbols:
                        continue
                    amt = float(p.get("contracts") or p.get("positionAmt") or p.get("amount") or 0.0)
                    side = str(p.get("side") or "").lower()
                    if side == "short":
                        amt = -abs(amt)
                    out[(s, "BTCC_PERP")] = float(amt)
        except Exception:
            pass
        return out

    async def fetch_positions_risk(self, symbols: list[str]) -> Dict[str, dict]:
        """Fetch perps position risk metrics (best-effort).

        Returns mapping sym -> {qty, mark, liq, margin_ratio, raw}
        """
        out: Dict[str, dict] = {}
        try:
            if hasattr(self.ex, "fetch_positions"):
                pos = await self.ex.fetch_positions(symbols)
                for p in pos or []:
                    s = str(p.get("symbol") or "")
                    if s not in symbols:
                        continue
                    info = p.get("info") or {}
                    qty = float(p.get("contracts") or p.get("positionAmt") or p.get("amount") or 0.0)
                    side = str(p.get("side") or "").lower()
                    if side == "short":
                        qty = -abs(qty)

                    mark = float(p.get("markPrice") or p.get("mark") or info.get("markPrice") or info.get("mark_price") or 0.0)
                    liq = float(p.get("liquidationPrice") or p.get("liquidation") or info.get("liquidationPrice") or info.get("liquidation_price") or 0.0)
                    mr = float(p.get("marginRatio") or info.get("marginRatio") or info.get("margin_ratio") or 0.0)
                    out[s] = {"qty": float(qty), "mark": float(mark), "liq": float(liq), "margin_ratio": float(mr), "raw": p}
        except Exception:
            pass
        return out

    async def fetch_balance(self) -> dict:
        try:
            if hasattr(self.ex, "fetch_balance"):
                return await self.ex.fetch_balance()
        except Exception:
            return {}
        return {}


class RouterBroker:
    def __init__(self, paper: PaperBroker, spot: MultiVenueSpotBroker, btcc: Optional[BTCCPerpsBroker] = None):
        self.paper = paper
        self.spot = spot
        self.btcc = btcc

    def _record_latency(self, venue: str, method: str, elapsed_ms: float) -> None:
        """Track API latency as an EWMA per venue/method."""
        try:
            lat = STATE.setdefault("latency_ms", {})
            v = lat.setdefault(str(venue), {})
            prev = float(v.get(method, elapsed_ms))
            alpha = float(CONFIG.get("LATENCY_EWMA_ALPHA", 0.2))
            v[method] = (1.0 - alpha) * prev + alpha * float(elapsed_ms)
        except Exception:
            pass

    async def _timed(self, venue: str, method: str, coro):
        t0 = time.time()
        res = await coro
        dt_ms = (time.time() - t0) * 1000.0
        self._record_latency(venue, method, dt_ms)
        return res

    async def execute(self, sym: str, side: Side, units: float, price: float, venue: Optional[str] = None, exec_style: str = "TAKER", meta: dict | None = None) -> None:
        mode = str(CONFIG.get("MODE","shadow")).lower()
        if mode in ("paper","shadow"):
            return await self.paper.execute(sym, side, units, price, venue=venue or "PAPER", exec_style=exec_style, meta=meta)

        # live mode
        # Explicit venues
        if venue == "BTCC_PERP":
            if not self.btcc:
                log.error("[ROUTER] BTCC perps broker missing")
                return
            return await self._timed("BTCC_PERP", "execute", self.btcc.execute(sym, side, units, price, venue="BTCC_PERP", exec_style=exec_style, meta=meta))
        if venue == "BTCC_SPOT":
            return await self._timed("BTCC_SPOT", "execute", self.spot.execute(sym, side, units, price, venue="BTCC_SPOT", exec_style=exec_style, meta=meta))

        # Auto routing: shorts to BTCC perps if configured, else spot venue
        if side == "SHORT" and bool(CONFIG.get("BTCC_PERPS_ONLY_FOR_SHORT", True)):
            if not self.btcc:
                log.error("[ROUTER] BTCC broker missing (cannot SHORT)")
                return
            return await self._timed("BTCC_PERP", "execute", self.btcc.execute(sym, side, units, price, venue="BTCC_PERP", exec_style=exec_style, meta=meta))

        # LONG (spot) routing
        spot_pref = str(CONFIG.get("SPOT_ROUTER_DEFAULT", "BINANCEUS")).upper()
        # In absence of a venue, use default
        chosen = spot_pref if venue in (None, "AUTO_SPOT", "AUTO_SHORT") else str(venue).upper()
        if chosen not in ("COINBASE", "KRAKEN", "BINANCEUS", "BTCC_SPOT"):
            chosen = spot_pref
        return await self._timed(chosen, "execute", self.spot.execute(sym, side, units, price, venue=chosen, exec_style=exec_style, meta=meta))

    async def snapshot_positions(self, symbols: list[str]) -> Dict[tuple[str, str], float]:
        """Institutional requirement: reconcile live positions each loop.

        In shadow/paper, we return an empty dict (planner uses internal state).
        In live, we best-effort pull positions from each venue and perps broker.
        """
        mode = str(CONFIG.get("MODE", "shadow")).lower()
        if mode in ("paper", "shadow"):
            return {}

        out: Dict[tuple[str, str], float] = {}
        # Spot venues
        for venue in ("COINBASE", "KRAKEN", "BINANCEUS", "BTCC_SPOT"):
            try:
                snap = await self.spot.fetch_positions(symbols, venue=venue)
                out.update(snap)
            except Exception:
                pass
        # Perps
        if self.btcc is not None:
            try:
                snap = await self.btcc.fetch_positions(symbols)
                out.update(snap)
            except Exception:
                pass
        return out

    async def cancel_replace(self, sym: str, venue: str, client_oid_prefix: str) -> int:
        """Cancel all open orders matching our client OID prefix.

        This is the core OMS cancel/replace primitive. Exchanges differ in whether they
        surface clientOrderId; we match on multiple common fields.
        Returns number of canceled orders.
        """
        mode = str(CONFIG.get("MODE", "shadow")).lower()
        if mode in ("paper", "shadow"):
            return 0

        canceled = 0
        try:
            if venue == "BTCC_PERP" and self.btcc is not None and hasattr(self.btcc.ex, "fetch_open_orders"):
                opens = await self.btcc.ex.fetch_open_orders(sym)
                for o in opens or []:
                    cid = str(o.get("clientOrderId") or o.get("clientOrderID") or o.get("id") or "")
                    if client_oid_prefix and client_oid_prefix not in cid:
                        continue
                    oid = str(o.get("id") or "")
                    if oid:
                        try:
                            await self.btcc.ex.cancel_order(oid, sym)
                            canceled += 1
                        except Exception:
                            pass
                return canceled

            # Spot venues
            opens = await self.spot.fetch_open_orders(sym, venue=venue)
            for o in opens or []:
                info = o.get("info") or {}
                cid = str(o.get("clientOrderId") or o.get("clientOrderID") or info.get("clientOrderId") or info.get("client_order_id") or "")
                # Some venues don't expose client OID; fall back to matching our prefix in "id".
                if not cid:
                    cid = str(o.get("id") or "")
                if client_oid_prefix and client_oid_prefix not in cid:
                    continue
                oid = str(o.get("id") or "")
                if oid and await self.spot.cancel_order(oid, sym, venue=venue):
                    canceled += 1
        except Exception:
            return canceled
        return canceled

    async def fetch_orderbook(self, sym: str, venue: str, limit: int = 50) -> dict:
        mode = str(CONFIG.get("MODE", "shadow")).lower()
        if mode in ("paper", "shadow"):
            return {}
        try:
            if venue == "BTCC_PERP" and self.btcc is not None and hasattr(self.btcc.ex, "fetch_order_book"):
                return await self._timed("BTCC_PERP", "fetch_orderbook", self.btcc.ex.fetch_order_book(sym, limit=limit))
            return await self._timed(str(venue), "fetch_orderbook", self.spot.fetch_orderbook(sym, venue=venue, limit=limit))
        except Exception:
            return {}

    async def fetch_order(self, order_id: str, sym: str, venue: str) -> dict:
        mode = str(CONFIG.get('MODE','shadow')).lower()
        if mode in ('paper','shadow'):
            return {}
        try:
            if venue == 'BTCC_PERP' and self.btcc is not None and hasattr(self.btcc.ex, 'fetch_order'):
                return await self._timed('BTCC_PERP', 'fetch_order', self.btcc.ex.fetch_order(order_id, sym))
            return await self._timed(str(venue), 'fetch_order', self.spot.fetch_order(order_id, sym, venue=venue))
        except Exception:
            return {}

    async def fetch_open_orders(self, sym: str, venue: str) -> list[dict]:
        mode = str(CONFIG.get('MODE','shadow')).lower()
        if mode in ('paper','shadow'):
            return []
        try:
            if venue == 'BTCC_PERP' and self.btcc is not None and hasattr(self.btcc.ex, 'fetch_open_orders'):
                return await self._timed('BTCC_PERP', 'fetch_open_orders', self.btcc.ex.fetch_open_orders(sym))
            return await self._timed(str(venue), 'fetch_open_orders', self.spot.fetch_open_orders(sym, venue=venue))
        except Exception:
            return []

    async def cancel_order(self, order_id: str, sym: str, venue: str) -> bool:
        mode = str(CONFIG.get('MODE','shadow')).lower()
        if mode in ('paper','shadow'):
            return False
        try:
            if venue == 'BTCC_PERP' and self.btcc is not None and hasattr(self.btcc.ex, 'cancel_order'):
                await self._timed('BTCC_PERP', 'cancel_order', self.btcc.ex.cancel_order(order_id, sym))
                return True
            return await self._timed(str(venue), 'cancel_order', self.spot.cancel_order(order_id, sym, venue=venue))
        except Exception:
            return False


    async def fetch_my_trades(self, sym: str, venue: str, since_ms: int | None = None, limit: int = 200) -> list[dict]:
        mode = str(CONFIG.get("MODE", "shadow")).lower()
        if mode in ("paper", "shadow"):
            return []
        try:
            if venue == "BTCC_PERP" and self.btcc is not None:
                if hasattr(self.btcc.ex, "fetch_my_trades"):
                    return await self._timed('BTCC_PERP', 'fetch_my_trades', self.btcc.ex.fetch_my_trades(sym, since=since_ms, limit=limit))
                return []
            return await self._timed(str(venue), 'fetch_my_trades', self.spot.fetch_my_trades(sym, venue=venue, since_ms=since_ms, limit=limit))
        except Exception:
            return []


    async def fetch_orders(self, sym: str, venue: str, since_ms: int | None = None, limit: int = 200) -> list[dict]:
        """Fetch order history (open+closed) best-effort."""
        mode = str(CONFIG.get("MODE", "shadow")).lower()
        if mode in ("paper", "shadow"):
            return []
        try:
            if venue == "BTCC_PERP" and self.btcc is not None and hasattr(self.btcc.ex, "fetch_orders"):
                return await self._timed("BTCC_PERP", "fetch_orders", self.btcc.ex.fetch_orders(sym, since=since_ms, limit=limit))
            ex = self.spot._get_ex(venue)  # type: ignore[attr-defined]
            if ex is not None and hasattr(ex, "fetch_orders"):
                return await self._timed(str(venue), "fetch_orders", ex.fetch_orders(sym, since=since_ms, limit=limit))
        except Exception:
            return []
        return []

    async def fetch_closed_orders(self, sym: str, venue: str, since_ms: int | None = None, limit: int = 200) -> list[dict]:
        """Fetch closed orders best-effort."""
        mode = str(CONFIG.get("MODE", "shadow")).lower()
        if mode in ("paper", "shadow"):
            return []
        try:
            if venue == "BTCC_PERP" and self.btcc is not None and hasattr(self.btcc.ex, "fetch_closed_orders"):
                return await self._timed("BTCC_PERP", "fetch_closed_orders", self.btcc.ex.fetch_closed_orders(sym, since=since_ms, limit=limit))
            ex = self.spot._get_ex(venue)  # type: ignore[attr-defined]
            if ex is not None and hasattr(ex, "fetch_closed_orders"):
                return await self._timed(str(venue), "fetch_closed_orders", ex.fetch_closed_orders(sym, since=since_ms, limit=limit))
        except Exception:
            return []
        return []
