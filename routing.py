from __future__ import annotations

from typing import Dict, Any, Optional, Tuple, List

from .config import CONFIG


def _resolve_fee_row(venue: str) -> Dict[str, Any]:
    """Return a fee row for venue.

    Supports two config shapes:

    1) Simple:
       FEES = { "COINBASE": {"maker":0.001, "taker":0.002}, ... }

    2) Tiered (30d volume driven):
       FEES = {
         "COINBASE": {
           "active_30d_usd": 250000,
           "tiers": [
             {"min_30d_usd":0, "maker":0.0010, "taker":0.0020},
             {"min_30d_usd":100000, "maker":0.0007, "taker":0.0016}
           ]
         }
       }

    You can also pin a tier explicitly:
      FEE_TIER_OVERRIDE = { "COINBASE": 1 }
    """
    fees = CONFIG.get("FEES", {}) or {}
    row = (fees or {}).get(venue, {}) or {}
    if "tiers" not in row:
        return row

    tiers = row.get("tiers") or []
    try:
        tiers = sorted(tiers, key=lambda t: float(t.get("min_30d_usd", 0.0)))
    except Exception:
        pass

    override = (CONFIG.get("FEE_TIER_OVERRIDE", {}) or {}).get(venue, None)
    if override is not None:
        try:
            return dict(tiers[int(override)])
        except Exception:
            return dict(tiers[-1]) if tiers else {}

    vol = float(row.get("active_30d_usd", CONFIG.get("ACTIVE_30D_USD", 0.0)) or 0.0)
    chosen = None
    for t in tiers:
        if vol >= float(t.get("min_30d_usd", 0.0) or 0.0):
            chosen = t
    return dict(chosen) if chosen else (dict(tiers[0]) if tiers else {})


def fee(venue: str, style: str) -> float:
    row = _resolve_fee_row(venue)
    if style.upper() == "MAKER":
        return float(row.get("maker", row.get("taker", 0.0)) or 0.0)
    return float(row.get("taker", row.get("maker", 0.0)) or 0.0)


def _slippage_proxy(venue: str, notional: float) -> float:
    """Deterministic fallback slippage model (fraction)."""
    per = (CONFIG.get("SLIPPAGE_BPS_PER_10K", {}) or {}).get(venue, 3.0)
    bps = float(per) * (float(notional) / 10000.0)
    return max(0.0, bps / 10000.0)


def _mid_from_ob(ob: dict) -> float:
    bids = ob.get("bids") or []
    asks = ob.get("asks") or []
    if not bids or not asks:
        return 0.0
    return (float(bids[0][0]) + float(asks[0][0])) / 2.0


def _avg_fill_price_from_ob(ob: dict, side: str, qty: float) -> float:
    if qty <= 0:
        return 0.0
    levels = (ob.get("asks") or []) if side.upper() in ("BUY", "LONG") else (ob.get("bids") or [])
    if not levels:
        return 0.0
    remaining = float(qty)
    cost = 0.0
    for px, sz in levels:
        take = min(remaining, float(sz))
        cost += take * float(px)
        remaining -= take
        if remaining <= 1e-12:
            break
    filled = qty - remaining
    if filled <= 1e-12:
        return 0.0
    return cost / filled


def _l2_slippage_frac(ob: dict, side: str, qty: float) -> float:
    """Estimated instantaneous slippage vs mid from L2 depth (fraction)."""
    mid = _mid_from_ob(ob)
    if mid <= 0:
        return 0.0
    avg = _avg_fill_price_from_ob(ob, side, qty)
    if avg <= 0:
        return 0.0
    return abs(avg - mid) / mid


def best_spot_venue(symbol: str, side: str, bundle: Dict[str, Any], style: str = "MAKER", notional: float = 0.0) -> str:
    """Legacy deterministic routing (no live orderbook)."""
    mkt = (bundle.get("market", {}) or {}).get(symbol, {}) or {}
    venues = (mkt.get("venues", {}) or {})
    candidates: List[Tuple[float, str]] = []
    for v in ("COINBASE", "KRAKEN", "BINANCEUS"):
        if not bool(CONFIG.get(f"{v}_ENABLED", True)):
            continue
        r = venues.get(v)
        if not r:
            continue
        bid = float(r.get("bid") or 0.0)
        ask = float(r.get("ask") or 0.0)
        last = float(r.get("price") or 0.0)
        mid = (bid + ask) / 2.0 if bid > 0 and ask > 0 else last
        if mid <= 0:
            continue
        spread = (ask - bid) / mid if bid > 0 and ask > 0 else float(CONFIG.get("SPREAD_FALLBACK", 0.0008))
        eff = fee(v, style) + max(0.0, spread) / 2.0 + _slippage_proxy(v, notional=notional)
        candidates.append((eff, v))
    if not candidates:
        return str(CONFIG.get("SPOT_ROUTER_DEFAULT", "BINANCEUS")).upper()
    candidates.sort(key=lambda x: x[0])
    return candidates[0][1]


async def best_spot_venue_l2(broker, *, symbol: str, side: str, qty: float, style: str = "MAKER", fallback_bundle: Optional[Dict[str, Any]] = None) -> str:
    """Institutional routing: route by expected effective cost using live L2.

    EffectiveCost ≈ fee(style) + spread/2 + L2_slippage(qty)

    If order books are unavailable, falls back to `best_spot_venue` (spread+proxy).
    """
    candidates: List[Tuple[float, str]] = []
    ob_levels = int(CONFIG.get("ORDERBOOK_LEVELS", 25))
    for v in ("COINBASE", "KRAKEN", "BINANCEUS"):
        if not bool(CONFIG.get(f"{v}_ENABLED", True)):
            continue
        try:
            ob = await broker.fetch_orderbook(symbol, v, limit=ob_levels)
        except Exception:
            ob = {}
        mid = _mid_from_ob(ob)
        bids = ob.get("bids") or []
        asks = ob.get("asks") or []
        if mid > 0 and bids and asks:
            spread = (float(asks[0][0]) - float(bids[0][0])) / mid
            sl = _l2_slippage_frac(ob, side, qty)
            eff = fee(v, style) + max(0.0, spread) / 2.0 + sl
            candidates.append((eff, v))
        else:
            # If no ob, fall back to bundle-based route later
            continue

    if candidates:
        candidates.sort(key=lambda x: x[0])
        return candidates[0][1]

    if fallback_bundle is not None:
        # approximate notional using index price from bundle
        price = float((((fallback_bundle.get("market", {}) or {}).get(symbol, {}) or {}).get("index", {}) or {}).get("price", 0.0))
        notional = abs(qty) * max(price, 1e-12)
        return best_spot_venue(symbol, side, fallback_bundle, style=style, notional=notional)

    return str(CONFIG.get("SPOT_ROUTER_DEFAULT", "BINANCEUS")).upper()
