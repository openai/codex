from __future__ import annotations
from dataclasses import dataclass
from typing import Dict, Any, Optional
from .config import CONFIG
from .utils import log, STATE
from .db import memorize
from .jury import judge_number

def clamp(v: float, lo: float, hi: float) -> float:
    return max(lo, min(hi, v))

@dataclass
class Proposal:
    changes: Dict[str, float]
    reason: str

def build_risk_proposal(metrics: Dict[str, float]) -> Optional[Proposal]:
    sharpe = float(metrics.get("sharpe", 0.0))
    dd = float(metrics.get("dd", float(STATE.get("drawdown", 0.0))))
    win = float(metrics.get("win_rate", 0.0))

    if dd > 0.18:
        return Proposal(
            changes={
                "MAX_EXPOSURE_PCT": float(CONFIG["MAX_EXPOSURE_PCT"]) * 0.85,
                "LEVERAGE": float(CONFIG["LEVERAGE"]) * 0.90,
            },
            reason=f"DD elevated ({dd:.2%}) => tighten"
        )
    if sharpe > 1.6 and win > 0.55 and dd < 0.10:
        return Proposal(
            changes={
                "MAX_EXPOSURE_PCT": float(CONFIG["MAX_EXPOSURE_PCT"]) * 1.10,
                "LEVERAGE": float(CONFIG["LEVERAGE"]) * 1.05,
            },
            reason=f"Strong metrics => cautiously loosen"
        )
    return None

def apply_changes(ch: Dict[str, float]) -> Dict[str, float]:
    applied: Dict[str, float] = {}
    if "LEVERAGE" in ch:
        v = float(ch["LEVERAGE"])
        v = clamp(v, float(CONFIG["LEVERAGE_MIN"]), float(CONFIG["LEVERAGE_MAX"]))
        v = min(v, float(CONFIG["MAX_LEVERAGE_CAP"]))  # HARD CAP <= 3x
        CONFIG["LEVERAGE"] = v
        applied["LEVERAGE"] = v
    if "MAX_EXPOSURE_PCT" in ch:
        v = float(ch["MAX_EXPOSURE_PCT"])
        v = clamp(v, float(CONFIG["MAX_EXPOSURE_MIN"]), float(CONFIG["MAX_EXPOSURE_MAX"]))
        CONFIG["MAX_EXPOSURE_PCT"] = v
        applied["MAX_EXPOSURE_PCT"] = v
    return applied

async def maybe_govern_risk(loop_idx: int, metrics: Dict[str, float], bot_conf: float) -> Dict[str, Any]:
    if not bool(CONFIG.get("RISK_GOV_ENABLED", True)):
        return {"did": False, "reason":"disabled"}
    interval = int(CONFIG.get("RISK_GOV_INTERVAL_LOOPS", 50))
    if interval <= 0 or loop_idx % interval != 0:
        return {"did": False, "reason":"not_time"}
    last = int(CONFIG.get("_RISK_GOV_LAST_LOOP", -10_000))
    if loop_idx - last < int(CONFIG.get("RISK_GOV_COOLDOWN_LOOPS", 50)):
        return {"did": False, "reason":"cooldown"}

    prop = build_risk_proposal(metrics)
    if not prop:
        return {"did": False, "reason":"no_proposal"}

    prompt = (
        "Return approval probability 0.0..1.0 for this risk change. Number only.\n\n"
        f"Proposal: {prop.changes}\nReason: {prop.reason}\nMetrics: {metrics}\n"
        "HARD CONSTRAINT: leverage must NEVER exceed 3x."
    )
    j = await judge_number(prompt)
    if int(j.get("n", 0)) < int(CONFIG.get("RISK_GOV_MIN_VOTES", 2)):
        return {"did": False, "reason":"insufficient_votes", "judges": j, "proposal": prop.changes}

    judges_score = float(j.get("mean", 0.0))
    combined = float(CONFIG.get("BOT_CONF_WEIGHT", 0.45))*bot_conf + float(CONFIG.get("JUDGES_CONF_WEIGHT", 0.55))*judges_score
    approved = combined >= float(CONFIG.get("RISK_GOV_APPROVAL", 0.67))

    rec = {"loop": loop_idx, "proposal": prop.changes, "reason": prop.reason, "metrics": metrics,
           "bot_conf": bot_conf, "judges": j, "combined": combined, "approved": approved}
    memorize("risk_governance", rec)

    if not approved:
        log.info(f"[GOV] rejected combined={combined:.2f}")
        return {"did": False, "reason":"rejected", **rec}

    applied = apply_changes(prop.changes)
    CONFIG["_RISK_GOV_LAST_LOOP"] = loop_idx
    log.warning(f"[GOV] applied {applied} combined={combined:.2f}")
    return {"did": True, "applied": applied, **rec}
