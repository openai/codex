from __future__ import annotations
from typing import Dict, Any, List
import numpy as np

def scan_universe(bundle: Dict[str, Any]) -> List[str]:
    """Universe filter.

    For the redesigned multi-venue schema we treat any symbol with a valid index price
    and at least one venue ticker as tradable.
    """
    market = bundle.get("market", {}) or {}
    out: List[str] = []
    for sym, row in market.items():
        idx = (row.get("index", {}) or {})
        p = float(idx.get("price", 0.0))
        venues = row.get("venues", {}) or {}
        if p > 0 and len(venues) > 0:
            out.append(sym)
    return out

def score_symbol(sym: str, bundle: Dict[str, Any]) -> float:
    """Legacy helper (kept for compatibility)."""
    market = bundle.get("market", {}) or {}
    row = market.get(sym, {}) or {}
    tfs = ((row.get("index", {}) or {}).get("tfs", {}) or {})
    mom = 0.0
    tf5 = tfs.get("5m")
    if isinstance(tf5, np.ndarray) and len(tf5) >= 2:
        c = tf5[:,4]
        mom = float((c[-1]/max(1e-9,c[-2])) - 1.0)

    sent = float((bundle.get("sentiment", {}) or {}).get("sent", 0.0))
    tv = (bundle.get("tv_data", {}) or {}).get(sym, {}) or {}
    rec = str(((tv.get("screener", {}) or {}).get("RECOMMENDATION",""))).upper()
    tv_bias = 0.15 if rec == "BUY" else (-0.15 if rec == "SELL" else 0.0)
    return float(2.0*mom + 0.4*sent + tv_bias)
