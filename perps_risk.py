from __future__ import annotations

from dataclasses import replace
from typing import Dict, Any, List, Tuple

from .config import CONFIG
from .utils import log
from .portfolio import Target


def _liq_distance_frac(mark: float, liq: float, qty: float) -> float:
    if mark <= 0 or liq <= 0 or qty == 0:
        return 0.0
    # long liquidates below mark; short liquidates above mark
    if qty > 0:
        return max(0.0, (mark - liq) / mark)
    return max(0.0, (liq - mark) / mark)


def enforce_perps_governor(targets: List[Target], *, perps_risk: Dict[str, Dict[str, Any]]) -> List[Target]:
    """Clamp / disable BTCC_PERP targets based on liquidation distance & margin.

    Inputs:
      perps_risk: sym -> {qty, mark, liq, margin_ratio, raw}

    Policies (config):
      - PERP_REQUIRE_RISK_DATA (default True): if no liq/mark, zero out perps targets.
      - PERP_MIN_LIQ_DISTANCE_PCT (default 0.06): require distance from liquidation.
      - PERP_MAX_MARGIN_RATIO (default 0.75): if margin_ratio is provided and exceeds threshold -> disable perps.

    This is intentionally conservative.
    """
    if not targets:
        return targets

    require = bool(CONFIG.get("PERP_REQUIRE_RISK_DATA", True))
    min_dist = float(CONFIG.get("PERP_MIN_LIQ_DISTANCE_PCT", 0.06))
    max_mr = float(CONFIG.get("PERP_MAX_MARGIN_RATIO", 0.75))

    out: List[Target] = []
    for t in targets:
        if str(t.venue).upper() != "BTCC_PERP":
            out.append(t)
            continue

        r = perps_risk.get(t.symbol) or {}
        mark = float(r.get("mark") or 0.0)
        liq = float(r.get("liq") or 0.0)
        mr = float(r.get("margin_ratio") or 0.0)

        # If we don't have risk data, fail closed.
        if require and (mark <= 0 or liq <= 0):
            log.warning(f"[perps-risk] missing mark/liq for {t.symbol}; disabling perps target")
            continue

        if mr > 0 and mr >= max_mr:
            log.warning(f"[perps-risk] margin_ratio high for {t.symbol} mr={mr:.3f}; disabling perps target")
            continue

        # Use current position sign if available; else assume target sign.
        pos_qty = float(r.get("qty") or 0.0)
        sign_qty = pos_qty if abs(pos_qty) > 1e-12 else float(t.qty)
        dist = _liq_distance_frac(mark, liq, sign_qty)
        if dist > 0 and dist < min_dist:
            # Scale down rather than hard drop (optional)
            scale = max(0.0, dist / max(min_dist, 1e-12))
            if scale <= 0.05:
                log.warning(f"[perps-risk] liq distance too small for {t.symbol} dist={dist:.3f}; disabling")
                continue
            log.warning(f"[perps-risk] liq distance clamp for {t.symbol} dist={dist:.3f} scale={scale:.3f}")
            out.append(replace(t, qty=t.qty * scale))
        else:
            out.append(t)

    return out
