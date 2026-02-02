"""Execution analytics (v15).

Compute per-venue/per-symbol execution KPIs and store them in exec_stats.
"""

from __future__ import annotations

import time
from collections import defaultdict
from typing import Any, Dict, Tuple, List

from . import db


def _f(x: Any, default: float = 0.0) -> float:
    try:
        return float(x)
    except Exception:
        return float(default)


def compute_and_store_exec_stats(window_minutes: int = 60) -> None:
    """Compute KPIs over the last N minutes and store a snapshot.

    Uses oms_events_v2 (deduped) and the orders table meta fields.
    """
    now_ms = int(time.time() * 1000)
    start_ms = now_ms - int(window_minutes * 60 * 1000)

    # Pull recent deduped events
    events = []
    try:
        with db._conn() as c:
            rows = c.execute(
                "SELECT ts,venue,sym,event,data FROM oms_events_v2 WHERE ts>=? ORDER BY ts ASC",
                (int(start_ms),),
            ).fetchall()
        for r in rows:
            events.append({"ts": int(r[0]), "venue": r[1], "sym": r[2], "event": r[3], "data": r[4]})
    except Exception:
        return

    # Pull latest known order states (for fill_rate + slippage)
    orders = []
    try:
        with db._conn() as c:
            rows = c.execute(
                "SELECT venue,sym,qty,filled,meta FROM orders WHERE updated_ts>=?",
                (int(start_ms),),
            ).fetchall()
        for r in rows:
            orders.append({"venue": r[0], "sym": r[1], "qty": _f(r[2]), "filled": _f(r[3]), "meta": r[4]})
    except Exception:
        orders = []

    ack = defaultdict(int)
    rej = defaultdict(int)
    maker_ack = defaultdict(int)

    for e in events:
        k = (e["venue"], e["sym"])
        if e["event"] == "ACK":
            ack[k] += 1
            # crude detection: style embedded in JSON text
            if "\"style\": \"MAKER\"" in (e.get("data") or ""):
                maker_ack[k] += 1
        elif e["event"] == "REJECT":
            rej[k] += 1

    # Slippage + fill-rate from orders meta
    slip_sum = defaultdict(float)
    slip_n = defaultdict(int)
    fill_sum = defaultdict(float)
    fill_n = defaultdict(int)

    import json as _json
    for o in orders:
        k = (o["venue"], o["sym"])
        qty = max(1e-12, o["qty"])
        fill_sum[k] += max(0.0, min(1.0, o["filled"] / qty))
        fill_n[k] += 1
        try:
            meta = _json.loads(o.get("meta") or "{}")
            exec_price = _f(meta.get("exec_price"), 0.0)
            mid = _f(meta.get("mid_at_submit"), 0.0)
            if exec_price > 0 and mid > 0:
                # Signed slippage: buy worse if exec_price > mid
                bps = (exec_price - mid) / mid * 10000.0
                slip_sum[k] += bps
                slip_n[k] += 1
        except Exception:
            pass

    ts = now_ms
    keys = set(list(ack.keys()) + list(rej.keys()) + list(fill_n.keys()) + list(slip_n.keys()))
    for venue, sym in keys:
        a = ack[(venue, sym)]
        r = rej[(venue, sym)]
        denom = max(1, a + r)
        maker_share = maker_ack[(venue, sym)] / max(1, a) if a > 0 else None
        reject_rate = r / denom if denom > 0 else None
        avg_slip = slip_sum[(venue, sym)] / slip_n[(venue, sym)] if slip_n[(venue, sym)] > 0 else None
        fill_rate = fill_sum[(venue, sym)] / fill_n[(venue, sym)] if fill_n[(venue, sym)] > 0 else None
        db.record_exec_stats(ts, venue, sym, maker_share=maker_share, reject_rate=reject_rate, avg_slippage_bps=avg_slip, fill_rate=fill_rate)
