
from __future__ import annotations
from dataclasses import dataclass
from typing import Dict, Any, List, Tuple
from .config import CONFIG
from .utils import clamp, log

@dataclass
class Target:
    symbol: str
    venue: str
    qty: float          # +long, -short
    sleeve: str
    meta: Dict[str, Any]

@dataclass
class Order:
    symbol: str
    venue: str
    side: str           # LONG/SHORT
    qty: float
    price: float
    reason: str
    sleeve: str
    meta: Dict[str, Any]

def _resolve_side(qty: float) -> str:
    return "LONG" if qty > 0 else "SHORT"

def aggregate_intents(intents: List[Dict[str, Any]], router_resolver) -> List[Target]:
    buckets: Dict[Tuple[str,str], Target] = {}
    min_notional = float(CONFIG.get("MIN_ORDER_NOTIONAL", 10.0))

    for it in intents:
        sym = str(it.get("symbol",""))
        side = str(it.get("side","")).upper()
        venue_hint = str(it.get("venue","AUTO_SPOT"))
        units = float(it.get("units",0.0))
        price = float(it.get("price",0.0))
        if not sym or side not in ("LONG","SHORT") or units <= 0 or price <= 0:
            continue

        resolved = router_resolver(sym, side, venue_hint=venue_hint, price=price, meta=it) or {}
        venue = str(resolved.get("venue", venue_hint))

        qty = units if side == "LONG" else -units
        key=(sym, venue)
        if key not in buckets:
            buckets[key]=Target(symbol=sym, venue=venue, qty=qty, sleeve=str(it.get("sleeve","")), meta={"sources":[it], "last_price":price})
        else:
            buckets[key].qty += qty
            buckets[key].meta["sources"].append(it)
            buckets[key].meta["last_price"]=price

    out=[]
    for t in buckets.values():
        p=float(t.meta.get("last_price",0.0))
        if abs(t.qty)*p >= min_notional:
            out.append(t)
    return out

def plan_rebalance(targets: List[Target], current: Dict[Tuple[str,str], float], prices: Dict[Tuple[str,str], float]) -> List[Order]:
    band_bps = float(CONFIG.get("REBALANCE_BAND_BPS", 25.0))
    min_notional = float(CONFIG.get("MIN_ORDER_NOTIONAL", 10.0))
    max_turnover_pct = float(CONFIG.get("MAX_TURNOVER_PCT", 0.25))
    equity = float(CONFIG.get("EQUITY_CACHE", 0.0)) or float(CONFIG.get("STARTING_EQUITY", 0.0))

    orders: List[Order]=[]
    turnover=0.0

    for t in targets:
        key=(t.symbol, t.venue)
        cur=float(current.get(key, 0.0))
        tgt=float(t.qty)
        price=float(prices.get(key, t.meta.get("last_price",0.0)))
        if price <= 0:
            continue

        band_qty = (band_bps/10000.0) * max(1e-12, abs(tgt))
        delta = tgt - cur
        if abs(delta) <= band_qty:
            continue

        notional = abs(delta)*price
        if notional < min_notional:
            continue

        turnover += notional
        orders.append(Order(
            symbol=t.symbol, venue=t.venue, side=_resolve_side(delta), qty=abs(delta), price=price,
            reason=f"rebalance_to_target band={band_bps}bps", sleeve=t.sleeve, meta=t.meta
        ))

    if equity > 0 and turnover > max_turnover_pct * equity:
        scale = (max_turnover_pct * equity) / max(turnover, 1e-12)
        log.warning(f"[planner] turnover capped scale={scale:.3f}")
        scaled=[]
        for o in orders:
            o.qty *= scale
            if o.qty*o.price >= min_notional:
                scaled.append(o)
        orders = scaled

    return orders
