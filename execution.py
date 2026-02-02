
from __future__ import annotations
import asyncio, random
from dataclasses import dataclass
import hashlib
from .config import CONFIG, is_armed
from .utils import log, STATE, clamp
from .db import mark_executed_order, record_oms_event
from .oms import OMSOrder, record_new, update_status, cancel_replace_with_status, make_client_oid

def _orderbook_mid(ob: dict) -> float:
    bids = ob.get("bids") or []
    asks = ob.get("asks") or []
    if not bids or not asks:
        return 0.0
    return (float(bids[0][0]) + float(asks[0][0]))/2.0

def estimate_avg_fill_price(ob: dict, side: str, qty: float) -> float:
    """Estimate average fill price for qty using level-2 order book."""
    if qty <= 0:
        return 0.0
    levels = (ob.get("asks") or []) if side == "LONG" else (ob.get("bids") or [])
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

def maker_limit_price(ob: dict, side: str, ref_price: float, urgency: float) -> float:
    """Choose a post-only maker price near top-of-book."""
    bids = ob.get("bids") or []
    asks = ob.get("asks") or []
    edge_bps = float(CONFIG.get("MAKER_EDGE_BPS", 2.0))
    # As urgency rises, reduce edge (be less picky) but remain maker-ish.
    edge_bps = max(0.0, edge_bps * (1.0 - 0.8*urgency))
    if side == "LONG":
        best_bid = float(bids[0][0]) if bids else ref_price
        px = best_bid * (1.0 - edge_bps/10000.0)
        return float(px)
    best_ask = float(asks[0][0]) if asks else ref_price
    px = best_ask * (1.0 + edge_bps/10000.0)
    return float(px)

@dataclass
class ExecDecision:
    urgency: float
    style: str
    max_child_notional: float
    child_delay_ms: int

def compute_urgency() -> float:
    dd=float(STATE.get("drawdown", 0.0))
    vol=float(STATE.get("vol_index", 0.0))
    dd0=float(CONFIG.get("URGENCY_DD_START", 0.10))
    dd1=float(CONFIG.get("URGENCY_DD_FULL", 0.20))
    u_dd = clamp((dd-dd0)/max(1e-9, dd1-dd0), 0.0, 1.0)
    v1=float(CONFIG.get("URGENCY_VOL_FULL", 0.08))
    u_vol = clamp(vol/max(1e-9, v1), 0.0, 1.0)
    return float(clamp(0.65*u_dd + 0.35*u_vol, 0.0, 1.0))

def decide_execution() -> ExecDecision:
    u = compute_urgency()
    style = "TAKER" if u >= float(CONFIG.get("MAKER_ONLY_URGENCY_MAX", 0.25)) else "MAKER"
    max_child=float(CONFIG.get("MAX_CHILD_ORDER_NOTIONAL", 200.0))*(1.0+2.0*u)
    delay=int(float(CONFIG.get("CHILD_ORDER_DELAY_MS", 250))*(1.0-0.6*u))
    return ExecDecision(urgency=u, style=style, max_child_notional=max_child, child_delay_ms=max(0, delay))


def _depth_notional(ob: dict, side: str, levels: int) -> float:
    """Estimate available notional in the first `levels` of the book.

    side=LONG means we care about asks (we will lift if we go taker),
    side=SHORT means bids.
    """
    lv = (ob.get("asks") or []) if side == "LONG" else (ob.get("bids") or [])
    if not lv:
        return 0.0
    tot = 0.0
    for px, sz in lv[: max(1, int(levels))]:
        tot += float(px) * float(sz)
    return float(tot)


async def _adaptive_child_qty(broker, order, exec_dec: ExecDecision, ref_price: float) -> float:
    """Compute a child quantity cap using live L2 depth.

    This is a light institutional approximation: cap each child to a fixed
    participation of top-of-book depth.
    """
    try:
        depth_lv = int(CONFIG.get("ADAPT_DEPTH_LEVELS", 10))
        part = float(CONFIG.get("MAX_PARTICIPATION_RATE", 0.10))
        ob = await broker.fetch_orderbook(order.symbol, order.venue, limit=int(CONFIG.get("ORDERBOOK_LEVELS", 25)))
        dnot = _depth_notional(ob, order.side, depth_lv)
        if dnot <= 0:
            return exec_dec.max_child_notional / max(ref_price, 1e-12)
        cap_notional = max(1.0, part * dnot)
        cap_notional = min(cap_notional, exec_dec.max_child_notional)
        return float(cap_notional / max(ref_price, 1e-12))
    except Exception:
        return exec_dec.max_child_notional / max(ref_price, 1e-12)

async def execute_with_retries(broker, order, exec_dec: ExecDecision) -> None:
    max_retries=int(CONFIG.get("EXEC_MAX_RETRIES", 3))
    backoff=float(CONFIG.get("EXEC_BACKOFF_BASE", 0.4))

    remaining=float(order.qty)
    price=float(order.price)
    # Base notional chop cap
    base_child_qty = exec_dec.max_child_notional / max(price, 1e-12)
    child=0

    # Deterministic base id for idempotency.
    base = f"{STATE.get('loop',0)}|{order.symbol}|{order.venue}|{order.side}|{order.qty:.8f}|{order.price:.8f}|{order.reason}|{order.sleeve}"
    base_id = hashlib.sha1(base.encode('utf-8')).hexdigest()[:16]

    # Smart execution (institutional): support TWAP/VWAP child scheduling.
    # - exec_algo: AUTO | TWAP | VWAP
    # - exec_horizon_sec: total time to complete child orders
    algo = str((order.meta or {}).get("exec_algo", "AUTO")).upper()
    horizon = float((order.meta or {}).get("exec_horizon_sec", CONFIG.get("EXEC_HORIZON_SEC", 0)) or 0)
    vwap_weights = (order.meta or {}).get("vwap_weights")

    # OMS cancel/replace: for maker-style executions, cancel prior open orders
    # with our deterministic prefix so we don't stack orders.
    if exec_dec.style == "MAKER" and bool(CONFIG.get("OMS_CANCEL_REPLACE", True)):
        try:
            await cancel_replace_with_status(broker, sym=order.symbol, venue=order.venue, prefix=base_id, sleeve=order.sleeve)
        except Exception:
            pass

    # Build slice plan
    slices = []  # list[(qty, sleep_ms_after)]
    if algo in ("TWAP","VWAP") and horizon > 0:
        import math
        n = int(math.ceil((abs(order.qty)*price) / max(exec_dec.max_child_notional, 1e-9)))
        n = max(1, min(n, int(CONFIG.get("EXEC_MAX_SLICES", 12))))
        if algo == "VWAP" and isinstance(vwap_weights, list) and len(vwap_weights) == n:
            w = [max(0.0, float(x)) for x in vwap_weights]
        else:
            # Fallback: TWAP-equivalent weights if no profile supplied.
            w = [1.0]*n
        s = sum(w) if sum(w) > 0 else float(n)
        w = [x/s for x in w]
        delays = 0.0 if n <= 1 else horizon/(n-1)
        qty_total = float(order.qty)
        for i in range(n):
            q = qty_total * w[i]
            slices.append((q, int(delays*1000) if i < n-1 else 0))
    else:
        # Default: notional-based chopping using ExecDecision with adaptive L2 cap.
        while remaining > 1e-12:
            dyn_cap = await _adaptive_child_qty(broker, order, exec_dec, ref_price=price)
            max_child_qty = min(base_child_qty, max(1e-12, float(dyn_cap)))
            q = remaining if remaining <= max_child_qty else max_child_qty
            remaining -= q
            slices.append((q, exec_dec.child_delay_ms if remaining > 1e-12 else 0))

    # v15: fill-feedback adaptive slicing.
    # We scale planned slice sizes based on observed fill ratio of previous slices.
    # This reduces market impact and reject rates and improves completion quality.
    feedback_enabled = bool(CONFIG.get("EXEC_FEEDBACK_ADAPT", True))
    target_fill = float(CONFIG.get("EXEC_TARGET_FILL_RATIO", 0.70))
    adapt_alpha = float(CONFIG.get("EXEC_FILL_ADAPT_ALPHA", 0.35))
    min_mult = float(CONFIG.get("EXEC_MIN_SLICE_MULT", 0.30))
    max_mult = float(CONFIG.get("EXEC_MAX_SLICE_MULT", 1.60))
    adaptive_mult = 1.0
    # Track remaining quantity so we can re-normalize when we scale slices.
    parent_remaining = float(sum(q for q, _ in slices))

    for idx, (q_plan, sleep_ms) in enumerate(slices, start=1):
        child += 1

        # Scale slice by adaptive multiplier; ensure we finish exactly.
        if feedback_enabled:
            q = float(q_plan) * float(adaptive_mult)
        else:
            q = float(q_plan)
        # Never exceed remaining parent qty; last slice closes remainder.
        if idx == len(slices):
            q = parent_remaining
        else:
            q = min(q, max(1e-12, parent_remaining))
        parent_remaining = max(0.0, parent_remaining - q)

        order_id = f"{base_id}-c{child}"
        client_oid = make_client_oid(f"{base_id}|{order.sleeve}", child)

        # ===== Trade permission gate (institutional fail-closed) =====
        # One codepath, three modes (paper/pilot/live). In pilot/live we require
        # explicit arming, otherwise we never send real orders.
        mode = str(CONFIG.get("TRADE_MODE", CONFIG.get("MODE", "paper"))).lower()
        if mode in ("pilot", "live") and not is_armed():
            try:
                record_new(OMSOrder(client_oid=client_oid, symbol=order.symbol, venue=order.venue, side=order.side, order_type=exec_dec.style, qty=q, price=price, sleeve=order.sleeve))
                update_status(client_oid, "REJECTED", meta={"blocked": True, "reason": "not_armed", "mode": mode})
                record_oms_event(client_oid, order.venue, order.symbol, "BLOCKED", {"reason": "not_armed", "mode": mode})
            except Exception:
                pass
            log.warning(f"[exec] BLOCKED (not armed) mode={mode} {order.side} {order.symbol} {order.venue} qty={q:.6f}")
            continue
        # Persist order lifecycle (institutional OMS)
        record_new(OMSOrder(client_oid=client_oid, symbol=order.symbol, venue=order.venue, side=order.side, order_type=exec_dec.style, qty=q, price=price, sleeve=order.sleeve))
        record_oms_event(client_oid, order.venue, order.symbol, "NEW", {"side": order.side, "qty": float(q), "price": float(price), "sleeve": order.sleeve, "algo": algo})
        # If DB says we've already executed this logical child order, skip.
        ok_to_exec = mark_executed_order(order_id, order.symbol, order.venue, order.side, q, price)
        if not ok_to_exec:
            log.warning(f"[exec] idempotency skip {order_id} {order.symbol} {order.venue} qty={q:.6f}")
            continue

        # For MAKER orders, compute a limit price using the live order book and
        # embed a stable client OID for idempotency and reconciliation.
        exec_price = price
        extra_meta = {}
        mid_submit = 0.0
        if exec_dec.style == "MAKER":
            try:
                ob = await broker.fetch_orderbook(order.symbol, order.venue, limit=int(CONFIG.get("ORDERBOOK_LEVELS", 25)))
                mid = _orderbook_mid(ob)
                ref = mid if mid > 0 else price
                mid_submit = float(mid) if mid > 0 else float(ref)
                exec_price = maker_limit_price(ob, order.side, ref_price=ref, urgency=exec_dec.urgency)
                extra_meta = {"client_oid": make_client_oid(f"{base_id}|{order.sleeve}", child), "post_only": bool(CONFIG.get("MAKER_POST_ONLY", True))}
            except Exception:
                exec_price = price

        for attempt in range(max_retries+1):
            try:
                resp = await broker.execute(
                    order.symbol,
                    order.side,
                    q,
                    float(exec_price),
                    venue=order.venue,
                    exec_style=exec_dec.style,
                    meta={"reason": order.reason, "sleeve": order.sleeve, "client_oid": client_oid, **extra_meta, **(order.meta or {})},
                )
                exid = ""
                if isinstance(resp, dict):
                    exid = str(resp.get("id") or resp.get("orderId") or "")
                update_status(client_oid, "OPEN", exchange_id=exid, filled=0.0, remaining=float(q), meta={"exec_price": float(exec_price), "style": exec_dec.style, "mid_at_submit": float(mid_submit)})
                record_oms_event(client_oid, order.venue, order.symbol, "ACK", {"exchange_order_id": exid, "exec_price": float(exec_price), "style": exec_dec.style, "mid_at_submit": float(mid_submit)})

                # v15 fill-feedback: sample fill ratio after a short delay and adapt next slices.
                if feedback_enabled and exid:
                    try:
                        fr = await _sample_fill_ratio(broker, venue=order.venue, symbol=order.symbol, exchange_order_id=exid, intended_qty=float(q))
                        # Update multiplier: if fill ratio is low, reduce slice sizes; if high, can increase.
                        # Multiplicative EWMA update in log-space keeps it stable.
                        # ratio_to_target < 1 => shrink
                        ratio_to_target = max(0.05, min(3.0, fr / max(1e-9, target_fill)))
                        # convert to multiplier update
                        proposed = adaptive_mult * (ratio_to_target ** adapt_alpha)
                        adaptive_mult = max(min_mult, min(max_mult, float(proposed)))
                        record_oms_event(client_oid, order.venue, order.symbol, "FILL_FEEDBACK", {"fill_ratio": float(fr), "adaptive_mult": float(adaptive_mult)})
                    except Exception:
                        pass
                break
            except Exception as e:
                if attempt >= max_retries:
                    log.error(f"[exec] failed {order.side} {order.symbol} {order.venue} qty={q:.6f}: {e}")
                    try:
                        update_status(client_oid, "REJECTED", meta={"error": str(e)})
                        record_oms_event(client_oid, order.venue, order.symbol, "REJECT", {"error": str(e)})
                    except Exception:
                        pass
                    break
                sleep = backoff*(2**attempt) + random.random()*0.15
                log.warning(f"[exec] retry {attempt+1}/{max_retries} {order.symbol} in {sleep:.2f}s: {e}")
                await asyncio.sleep(sleep)

        if sleep_ms > 0:
            await asyncio.sleep(sleep_ms/1000.0)


async def _sample_fill_ratio(broker, venue: str, symbol: str, exchange_order_id: str, intended_qty: float) -> float:
    """Best-effort fill-ratio sampler for adaptive slicing.

    Uses exchange order status if available; otherwise falls back to 1.0.
    This is deliberately conservative: if we can't observe fills, we avoid
    shrinking slices purely due to missing data.
    """
    try:
        # Small delay so exchanges have time to register partial fills.
        delay_ms = int(CONFIG.get("EXEC_FEEDBACK_SAMPLE_DELAY_MS", 350))
        if delay_ms > 0:
            await asyncio.sleep(delay_ms / 1000.0)

        # Broker is expected to expose fetch_order; RouterBroker does.
        if hasattr(broker, "fetch_order"):
            o = await broker.fetch_order(exchange_order_id, symbol, venue=venue)
            if isinstance(o, dict):
                filled = float(o.get("filled") or 0.0)
                amount = float(o.get("amount") or intended_qty or 0.0)
                denom = max(1e-12, amount)
                return max(0.0, min(1.0, filled / denom))
    except Exception:
        pass
    return 1.0
