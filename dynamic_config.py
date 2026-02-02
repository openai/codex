from __future__ import annotations
from typing import Dict, Any
from .config import CONFIG
from .utils import log

try:
    import optuna
except Exception:
    optuna = None  # type: ignore

def maybe_optuna_tune(loop_idx: int) -> Dict[str, Any]:
    if not bool(CONFIG.get("OPTUNA_ENABLED", False)):
        return {"did": False, "reason": "disabled"}
    if optuna is None:
        return {"did": False, "reason": "optuna_missing"}
    every = int(CONFIG.get("OPTUNA_EVERY_LOOPS", 300))
    if every <= 0 or loop_idx % every != 0:
        return {"did": False, "reason": "not_time"}

    # Minimal example: tune CONF_ENTRY_THRESHOLD in safe bounds
    def objective(trial):
        thr = trial.suggest_float("CONF_ENTRY_THRESHOLD", float(CONFIG["CONF_ENTRY_MIN"]), float(CONFIG["CONF_ENTRY_MAX"]))
        # placeholder: you'd backtest/score here
        return abs(thr - 0.72)
    study = optuna.create_study(direction="minimize")
    study.optimize(objective, n_trials=int(CONFIG.get("OPTUNA_TRIALS", 30)))
    best = study.best_params
    CONFIG.update(best)
    log.warning(f"[OPTUNA] updated params: {best}")
    return {"did": True, "best": best}
