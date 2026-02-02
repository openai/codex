from __future__ import annotations
from dataclasses import dataclass
from typing import Tuple
from .config import CONFIG

@dataclass
class KellySizer:
    avg_vol: float = 1.0
    avg_atr: float = 200.0

    def size(self, equity: float, entry: float, stop: float, conf: float, vol: float) -> Tuple[float, float]:
        loss = abs(entry - stop)
        if loss <= 0: return 0.0, 0.0
        if vol < 0.8 * self.avg_vol: return 0.0, 0.0
        frac = min(0.25, 0.05 + 0.35*float(conf))
        frac = max(0.01, min(frac, 0.25))
        lev = float(CONFIG.get("LEVERAGE", 1.0))
        max_expo = float(CONFIG.get("MAX_EXPOSURE_PCT", 0.05))
        notional_cap = equity * max_expo / max(1e-9, lev)
        units = (notional_cap / max(1e-9, loss)) * frac
        return float(max(0.0, units)), float(frac)
