from __future__ import annotations
import logging, time, json
from typing import Any, Dict
from .config import CONFIG

logging.basicConfig(level=logging.INFO, format="%(asctime)s | %(levelname)s | %(message)s")
log = logging.getLogger("aether_edge")

STATE: Dict[str, Any] = {
    "equity": float(CONFIG.get("STARTING_EQUITY", 0.0)),
    "drawdown": 0.0,
    "mode": str(CONFIG.get("MODE", "shadow")),
    "loop": 0,
    "last_decision": {},
}

def update_drawdown(start_equity: float) -> None:
    eq = float(STATE.get("equity", 0.0))
    dd = (start_equity - eq) / max(1e-9, start_equity)
    STATE["drawdown"] = max(float(STATE.get("drawdown", 0.0)), float(dd))

def safe_json(obj: Any) -> str:
    try:
        return json.dumps(obj, ensure_ascii=False, default=str)
    except Exception:
        return "{}"


def clamp(x: float, lo: float, hi: float) -> float:
    try:
        return float(max(lo, min(hi, float(x))))
    except Exception:
        return float(lo)
