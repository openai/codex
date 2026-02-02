
from __future__ import annotations
import asyncio, time
from typing import Dict, Any, List, Tuple
import numpy as np

import os
from .config import CONFIG, load_config, load_env_keys, apply_mode_profile, read_mode_request, is_armed
from .utils import log, STATE
from .data_sources import fetch_all
from .universe import scan_universe
from .strategy_combo import ComboEngine
from .broker import PaperBroker, BTCCPerpsBroker, MultiVenueSpotBroker, RouterBroker
from .ops import CircuitBreaker
from .db import memorize, record_trade, sleeve_pnl_snapshot
from .oms import reconcile
from .execution_analytics import compute_and_store_exec_stats
from .tv_webhook import start_tv_webhook_server
from .governance import maybe_govern_risk
from .dynamic_config import maybe_optuna_tune
from .portfolio import aggregate_intents, plan_rebalance
from .execution import decide_execution, execute_with_retries
from .routing import best_spot_venue, best_spot_venue_l2
from .risk import enforce_risk
from .analytics import mark_to_market

def _venue_resolver(sym: str, side: str, venue_hint: str, price: float, meta: Dict[str, Any], bundle: Dict[str, Any], exec_style: str) -> Dict[str, Any]:
    """Resolve AUTO_* venues into concrete venues.

    Institutional note: true best-ex routing requires live order books & fees. Here we provide
    deterministic routing with a clean extension point.
    """
    vh = str(venue_hint or "").upper()
    if vh in ("BTCC_PERP","BTCC_SPOT","COINBASE","KRAKEN","BINANCEUS"):
        return {"venue": vh}

    # Shorts: default to BTCC perps (configurable)
    if side == "SHORT" and bool(CONFIG.get("BTCC_PERPS_ONLY_FOR_SHORT", True)):
        return {"venue": "BTCC_PERP"}

    # Longs: best-ex among spot venues
    notional = float(abs(meta.get("units",0.0))*max(price,1e-12))
    v = best_spot_venue(sym, side=side, bundle=bundle, style=exec_style, notional=notional)
    return {"venue": v}

def _price_for_key(sym: str, venue: str, bundle: Dict[str, Any]) -> float:
    row = (bundle.get("market", {}) or {}).get(sym, {}) or {}
    if venue == "BTCC_PERP":
        return float((row.get("btcc_perp", {}) or {}).get("price", 0.0))
    if venue == "BTCC_SPOT":
        # if not present, fall back to index price
        return float((row.get("btcc_spot", {}) or {}).get("price", (row.get("index", {}) or {}).get("price", 0.0)))
    # spot venues use index price for planning
    return float((row.get("index", {}) or {}).get("price", 0.0))

async def trading_loop():
    # Load config first so mode/profile selection can shape runtime safety.
    cfg_path = str(os.getenv("AETHER_CONFIG", "config.json"))
    load_config(cfg_path)
    # Dashboard can request a trade mode; honor at startup (restart-safe).
    req = read_mode_request()
    if req:
        CONFIG["TRADE_MODE"] = req
    apply_mode_profile()
    # Fail-closed: if pilot/live requested but not armed, drop to paper.
    if str(CONFIG.get("TRADE_MODE", "paper")).lower() in ("pilot", "live") and not is_armed():
        from .utils import log as _log
        _log.warning("Mode requested via dashboard/config but session is not armed; forcing paper mode.")
        CONFIG["TRADE_MODE"] = "paper"
        apply_mode_profile()
    load_env_keys()
    STATE["mode"] = str(CONFIG.get("TRADE_MODE", CONFIG.get("MODE", "paper")))

    start_equity = float(CONFIG.get("STARTING_EQUITY", 0.0))
    STATE["equity"] = start_equity
    STATE["drawdown"] = 0.0
    STATE.setdefault("positions_by_venue", {})  # (sym|venue) -> qty

    start_tv_webhook_server()

    paper = PaperBroker()
    btcc = None
    if str(CONFIG.get("MODE","paper")).lower() == "live":
        try:
            btcc = BTCCPerpsBroker()
        except Exception as e:
            log.error(f"BTCC init failed: {e}")
    spot = MultiVenueSpotBroker(paper=paper)
    broker = RouterBroker(paper=paper, spot=spot, btcc=btcc)

    engine = ComboEngine()
    ops = CircuitBreaker()

    loop_idx = 0
    while True:
        loop_idx += 1
        STATE["loop"] = loop_idx

        bundle = await fetch_all()
        symbols = scan_universe(bundle)

        # Live reconciliation (institutional requirement): replace internal position cache
        # with best-effort venue snapshots. In shadow mode we keep the internal ledger.
        try:
            snap = await broker.snapshot_positions(symbols)
            if snap:
                STATE["positions_by_venue"] = {f"{s}|{v}": float(q) for (s, v), q in snap.items()}
        except Exception:
            pass

        # Perps risk snapshot (liq/margin) for BTCC_PERP governor
        try:
            if getattr(broker, "btcc", None) is not None:
                pr = await broker.btcc.fetch_positions_risk(symbols)  # type: ignore[attr-defined]
                if pr:
                    STATE["perps_risk"] = pr
        except Exception:
            pass


        # Prime OMS reconciliation: orders + fills + sleeve PnL attribution
        try:
            venues = ["COINBASE", "KRAKEN", "BINANCEUS", "BTCC_SPOT", "BTCC_PERP"]
            since_ms = (STATE.get("recon_since_ms", {}) or {}).get("global", 0) or None
            await reconcile(broker, symbols=symbols, venues=venues, since_ms=since_ms)
            # oms.reconcile stores latest timestamp in DB memory; keep a local hint too
            # (best-effort)
            STATE["recon_since_ms"] = {"global": int(time.time()*1000)}
        except Exception:
            pass

        # v15: execution analytics snapshot (desk KPIs)
        try:
            every = int(CONFIG.get("EXEC_STATS_INTERVAL_LOOPS", 5))
            if every > 0 and (int(STATE.get("loop", 0)) % every == 0):
                compute_and_store_exec_stats(window_minutes=int(CONFIG.get("EXEC_STATS_WINDOW_MIN", 60)))
        except Exception:
            pass

        # Decide execution style early so routing can use maker/taker fees
        exec_dec = decide_execution()

        # Update equity cache for the planner/risk
        STATE["equity"] = float(getattr(paper.port, "equity", STATE.get("equity", start_equity)))
        CONFIG["EQUITY_CACHE"] = float(STATE["equity"])

        # Decide raw intents (trend + carry). These are *targets*, not immediate orders.
        intents = engine.decide_portfolio(symbols=symbols, bundle=bundle, equity=float(STATE["equity"]))

        # Derive a conservative vol index for execution urgency (median vol of active signals)
        vols = [float(it.get("vol", 0.0)) for it in intents if it.get("sleeve") == "trend"]
        STATE["vol_index"] = float(np.median(vols)) if vols else float(STATE.get("vol_index", 0.0))

        # Convert intents -> netted targets (institutional: net to reduce churn)
        def resolver(sym: str, side: str, venue_hint: str, price: float, meta: Dict[str, Any]):
            return _venue_resolver(sym, side, venue_hint, price, meta, bundle=bundle, exec_style=exec_dec.style)

        targets = aggregate_intents(intents, router_resolver=resolver)

        # Current positions snapshot (best-effort). Keyed by (sym, venue).
        cur_raw: Dict[str, float] = STATE.get("positions_by_venue", {}) or {}
        current: Dict[Tuple[str,str], float] = {}
        for k, v in cur_raw.items():
            try:
                sym, venue = k.split("|", 1)
                current[(sym, venue)] = float(v)
            except Exception:
                continue

        # Prices per key (planning)
        prices: Dict[Tuple[str,str], float] = {}
        for t in targets:
            prices[(t.symbol, t.venue)] = _price_for_key(t.symbol, t.venue, bundle)

        # Institutional risk policy clamps targets before planning orders
        targets = enforce_risk(targets, prices=prices, bundle=bundle, equity=float(STATE["equity"]))

        # Plan delta orders (rebalance-to-target with bands + turnover cap)
        orders = plan_rebalance(targets, current=current, prices=prices)

        for o in orders:
            # Best-ex routing using live L2 (spot only)
            try:
                if bool(CONFIG.get("BESTEX_LIVE_ROUTING", True)) and str(o.venue).upper() in ("COINBASE","KRAKEN","BINANCEUS"):
                    v2 = await best_spot_venue_l2(broker, symbol=o.symbol, side=o.side, qty=o.qty, style=exec_dec.style, fallback_bundle=bundle)
                    o.venue = v2
            except Exception:
                pass

            await execute_with_retries(broker, o, exec_dec)

            # Update internal position book (best-effort fill assumption)
            key = f"{o.symbol}|{o.venue}"
            q = float(cur_raw.get(key, 0.0))
            q = q + (o.qty if o.side == "LONG" else -o.qty)
            cur_raw[key] = q
            STATE["positions_by_venue"] = cur_raw

            memorize("order", {"loop": loop_idx, "symbol": o.symbol, "venue": o.venue, "side": o.side, "qty": o.qty, "price": o.price, "sleeve": o.sleeve, "reason": o.reason})
            record_trade(o.symbol, o.venue, o.side, o.qty, o.price, sleeve=o.sleeve, reason=o.reason, meta=o.meta)
            STATE["last_order"] = {"symbol": o.symbol, "venue": o.venue, "side": o.side, "qty": o.qty, "price": o.price}

        # Sleeve PnL snapshot (audit)
        try:
            if loop_idx % int(CONFIG.get('PNL_SNAPSHOT_EVERY_LOOPS', 10)) == 0:
                snap = sleeve_pnl_snapshot()
                memorize('sleeve_pnl', snap)
            try:
                # Sleeve-level MTM + exposure decomposition
                mark_to_market(prices)
            except Exception:
                pass
        except Exception:
            pass

        # Governance + tuning hooks (fail-closed: adjust risk only; never force trades)
        bot_conf = float((STATE.get("last_order", {}) or {}).get("confidence", 0.5))
        metrics = {"sharpe": 1.6, "dd": float(STATE.get("drawdown", 0.0)), "win_rate": 0.55}
        await maybe_govern_risk(loop_idx, metrics, bot_conf)
        maybe_optuna_tune(loop_idx)

        hib = ops.maybe_hibernate()
        if hib > 0:
            log.warning(f"[OPS] hibernating {hib:.0f}s")
            await asyncio.sleep(hib)
            continue
        if ops.should_pause():
            break

        await asyncio.sleep(float(CONFIG.get("LOOP_SECONDS", 15)))

def main():
    asyncio.run(trading_loop())
