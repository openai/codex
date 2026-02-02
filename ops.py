from __future__ import annotations
from .config import CONFIG
from .utils import log, STATE

class CircuitBreaker:
    def should_pause(self) -> bool:
        if float(STATE.get("drawdown", 0.0)) > float(CONFIG.get("DD_PAUSE", 0.25)):
            log.warning("[OPS] drawdown pause")
            return True
        return False

    def maybe_hibernate(self) -> float:
        if float(STATE.get("drawdown", 0.0)) > float(CONFIG.get("HIBERNATE_DD", 0.30)):
            return float(CONFIG.get("HIBERNATE_SECONDS", 86400))
        return 0.0
