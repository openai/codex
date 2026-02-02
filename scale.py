from __future__ import annotations
from typing import Dict, Any
from .config import CONFIG

def leverage_for_symbol(sym: str, score: float) -> float:
    # Tiered leverage: bounded, never exceeds hard cap.
    base = float(CONFIG.get("LEVERAGE", 1.0))
    if score > 0.5:
        base *= 1.10
    elif score < -0.5:
        base *= 0.90
    cap = float(CONFIG.get("MAX_LEVERAGE_CAP", 3.0))
    return float(max(1.0, min(cap, base)))
