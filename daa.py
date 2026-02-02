from __future__ import annotations
from typing import Dict, Any

def dynamic_asset_allocation(signals: Dict[str, Any]) -> Dict[str, float]:
    """Stub DAA: convert per-symbol scores to weights. Extend as needed."""
    scores = {k: float(v) for k,v in signals.items() if isinstance(v, (int,float))}
    if not scores:
        return {}
    # softmax-like
    import math
    exps = {k: math.exp(max(-5, min(5, s))) for k,s in scores.items()}
    total = sum(exps.values()) + 1e-9
    return {k: v/total for k,v in exps.items()}
