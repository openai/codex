"""Venue microstructure normalization.

Institutional execution relies on always respecting venue rules:
 - tick size / price precision
 - min amount / min notional (min cost)

We fetch these via ccxt market metadata when available and apply a best-effort
rounding/validation pass before sending orders.

This module is intentionally conservative: if we can't validate, we do not
block the order, but we *do* round to sane precision.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
from typing import Any, Dict, Optional, Tuple


@dataclass
class MarketRules:
    price_tick: Optional[float] = None
    amount_step: Optional[float] = None
    min_amount: Optional[float] = None
    min_cost: Optional[float] = None
    price_precision: Optional[int] = None
    amount_precision: Optional[int] = None


def _tick_from_precision(px: float, precision: Optional[int]) -> Optional[float]:
    if precision is None:
        return None
    try:
        return 10 ** (-int(precision))
    except Exception:
        return None


def get_market_rules(market: Dict[str, Any]) -> MarketRules:
    limits = market.get("limits") or {}
    price_lim = limits.get("price") or {}
    amt_lim = limits.get("amount") or {}
    cost_lim = limits.get("cost") or {}
    precision = market.get("precision") or {}

    pr = MarketRules(
        price_tick=None,
        amount_step=None,
        min_amount=float(amt_lim.get("min")) if amt_lim.get("min") is not None else None,
        min_cost=float(cost_lim.get("min")) if cost_lim.get("min") is not None else None,
        price_precision=int(precision.get("price")) if precision.get("price") is not None else None,
        amount_precision=int(precision.get("amount")) if precision.get("amount") is not None else None,
    )

    # Some exchanges supply tick/step in "precision" only.
    pr.price_tick = _tick_from_precision(0.0, pr.price_precision)
    pr.amount_step = _tick_from_precision(0.0, pr.amount_precision)

    # Some markets expose "info" fields (varies by exchange). Best-effort.
    info = market.get("info") or {}
    for k in ("tickSize", "tick_size", "price_tick"):
        if pr.price_tick is None and info.get(k) is not None:
            try:
                pr.price_tick = float(info.get(k))
            except Exception:
                pass
    for k in ("stepSize", "step_size", "amount_step"):
        if pr.amount_step is None and info.get(k) is not None:
            try:
                pr.amount_step = float(info.get(k))
            except Exception:
                pass

    return pr


def _round_to_step(x: float, step: Optional[float], mode: str = "floor") -> float:
    if step is None or step <= 0:
        return float(x)
    n = x / step
    if mode == "ceil":
        return float(math.ceil(n) * step)
    if mode == "round":
        return float(round(n) * step)
    return float(math.floor(n) * step)


def normalize_order(
    *,
    price: float,
    qty: float,
    side: str,
    rules: MarketRules,
    min_notional_override: Optional[float] = None,
) -> Tuple[float, float, bool, str]:
    """Return (price, qty, ok, reason).

    ok=False means the order violates min amount/cost; callers can skip or bump.
    """
    px = float(price)
    q = float(qty)
    if px <= 0 or q <= 0:
        return px, q, False, "non_positive"

    # For safety, floor quantities and round prices toward safety.
    # Buy: rounding price up is safer (more likely fill), Sell: round down.
    px_mode = "ceil" if side == "LONG" else "floor"
    px = _round_to_step(px, rules.price_tick, mode=px_mode)
    q = _round_to_step(q, rules.amount_step, mode="floor")

    min_amt_ok = True
    if rules.min_amount is not None:
        min_amt_ok = q >= float(rules.min_amount) - 1e-12

    min_cost = min_notional_override if min_notional_override is not None else rules.min_cost
    min_cost_ok = True
    if min_cost is not None:
        min_cost_ok = (q * px) >= float(min_cost) - 1e-9

    ok = bool(min_amt_ok and min_cost_ok)
    reason = "ok" if ok else ("min_amount" if not min_amt_ok else "min_cost")
    return px, q, ok, reason
