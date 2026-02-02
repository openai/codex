from __future__ import annotations

import time
from typing import Dict, Tuple, Any

from .utils import log
from . import db


def mark_to_market(prices: Dict[Tuple[str, str], float], *, prefer_venue: str = "index") -> None:
    """Compute sleeve-level MTM PnL snapshots from current marks.

    We mark sleeve_positions (which are sleeve+symbol aggregates) using a conservative mark:
    - prefer (sym, prefer_venue) if present in prices
    - else use any venue price available for that symbol
    - else skip

    Writes rows into DB table `sleeve_mtm` keyed by current unix ts.
    """
    ts = int(time.time())
    pos = db.sleeve_pnl_snapshot()
    if not pos:
        return

    # Build symbol->mark lookup
    sym_mark: Dict[str, float] = {}
    for (sym, venue), px in prices.items():
        if px and px > 0:
            if venue == prefer_venue and sym not in sym_mark:
                sym_mark[sym] = float(px)

    # fallback: first seen
    for (sym, venue), px in prices.items():
        if px and px > 0 and sym not in sym_mark:
            sym_mark[sym] = float(px)

    wrote = 0
    for r in pos:
        sleeve = str(r.get("sleeve"))
        sym = str(r.get("sym"))
        qty = float(r.get("qty") or 0.0)
        avg = float(r.get("avg_cost") or 0.0)
        realized = float(r.get("realized_pnl") or 0.0)
        mark = float(sym_mark.get(sym, 0.0))
        if mark <= 0:
            continue
        unreal = qty * (mark - avg)
        try:
            db.record_sleeve_mtm(ts, sleeve, sym, qty, avg, mark, unreal, realized)
            wrote += 1
        except Exception:
            continue

    if wrote and wrote <= 20:
        log.debug(f"[mtm] wrote {wrote} sleeve marks @ts={ts}")
