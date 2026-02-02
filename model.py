from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Optional

from .config import CONFIG
from .utils import clamp

from . import ta
from . import sentiment
from . import tv_webhook
from .qml import qml_score as qml_score_fn
from .quantum_cortex import QuantumCortex


@dataclass
class Decision:
    action: int                  # -1 short, 0 hold, +1 long
    confidence: float            # 0..1
    directional_bias: float      # -1..1
    components: Dict[str, Any]


class PolicyEngine:
    """Fusion policy: TA + sentiment + patterns + TradingView + (QML + QuantCortex).

    Notes:
    - This engine produces a *suggested* action. Execution/risk governance still apply downstream.
    - 'QuantCortex' is intended to be your quantitative/market-structure brain (regimes, entropy guards, etc.).
    """

    def __init__(self):
        self.qcortex = QuantumCortex()

    def _compute_components(self, symbol: str, market: Dict[str, Any], tv: Optional[dict]) -> Dict[str, Any]:
        # Technical / pattern / sentiment
        tech_score, tech_conf = ta.calc_tech_conf(symbol, market)
        pat_score, pat_conf = ta.calc_pattern_conf(symbol, market)
        sent_score, sent_conf = sentiment.sentiment_conf(symbol, market)

        # TradingView
        tv_bias = 0.0
        tv_conf = 0.0
        if tv:
            tv_bias, tv_conf = tv_webhook.tv_bias_conf(symbol, tv)

        # QML
        qml_score, qml_conf = qml_score_fn(symbol, market)

        # Quantitative Cortex
        qc = self.qcortex.analyze(symbol=symbol, market=market)
        qc_score = float(qc.get("score", 0.0))
        qc_conf = float(qc.get("confidence", 0.0))

        # Clamp ranges
        tech_score = clamp(float(tech_score), -1.0, 1.0)
        pat_score = clamp(float(pat_score), -1.0, 1.0)
        sent_score = clamp(float(sent_score), -1.0, 1.0)
        tv_bias = clamp(float(tv_bias), -1.0, 1.0)
        qml_score = clamp(float(qml_score), -1.0, 1.0)
        qc_score = clamp(float(qc_score), -1.0, 1.0)

        tech_conf = clamp(float(tech_conf), 0.0, 1.0)
        pat_conf = clamp(float(pat_conf), 0.0, 1.0)
        sent_conf = clamp(float(sent_conf), 0.0, 1.0)
        tv_conf = clamp(float(tv_conf), 0.0, 1.0)
        qml_conf = clamp(float(qml_conf), 0.0, 1.0)
        qc_conf = clamp(float(qc_conf), 0.0, 1.0)

        return {
            "tech": {"score": tech_score, "conf": tech_conf},
            "pattern": {"score": pat_score, "conf": pat_conf},
            "sent": {"score": sent_score, "conf": sent_conf},
            "tv": {"score": tv_bias, "conf": tv_conf, "raw": tv},
            "qml": {"score": qml_score, "conf": qml_conf},
            "qcortex": {"score": qc_score, "conf": qc_conf, "raw": qc},
        }

    def decide(self, symbol: str, market: Dict[str, Any], tv: Optional[dict] = None) -> Decision:
        w_tech = float(CONFIG.get("W_TECH", 0.35))
        w_sent = float(CONFIG.get("W_SENT", 0.25))
        w_pat = float(CONFIG.get("W_PATTERN", 0.25))
        w_tv = float(CONFIG.get("W_TV", 0.15))
        w_qml = float(CONFIG.get("W_QML", 0.10))

        comps = self._compute_components(symbol, market, tv)

        # Signed directional bias (for action selection)
        bias = 0.0
        bias += w_tech * comps["tech"]["score"] * comps["tech"]["conf"]
        bias += w_pat * comps["pattern"]["score"] * comps["pattern"]["conf"]
        bias += w_sent * comps["sent"]["score"] * comps["sent"]["conf"]
        bias += w_tv * comps["tv"]["score"] * comps["tv"]["conf"]

        # Split quantum weight between QML and QuantCortex
        qml_part = comps["qml"]["score"] * comps["qml"]["conf"]
        qcx_part = comps["qcortex"]["score"] * comps["qcortex"]["conf"]
        bias += w_qml * (0.60 * qml_part + 0.40 * qcx_part)

        bias = clamp(bias, -1.0, 1.0)

        # Confidence: weighted mean of component confidences + magnitude of bias
        conf = 0.0
        conf += w_tech * comps["tech"]["conf"]
        conf += w_pat * comps["pattern"]["conf"]
        conf += w_sent * comps["sent"]["conf"]
        conf += w_tv * comps["tv"]["conf"]
        conf += w_qml * (0.60 * comps["qml"]["conf"] + 0.40 * comps["qcortex"]["conf"])
        conf = clamp(conf * 0.85 + abs(bias) * 0.15, 0.0, 1.0)

        # Entry gates
        entry_thresh = float(CONFIG.get("ENTRY_CONF_THRESH", 0.55))
        min_bias = float(CONFIG.get("ENTRY_BIAS_THRESH", 0.12))

        action = 0
        if conf >= entry_thresh and abs(bias) >= min_bias:
            action = 1 if bias > 0 else -1

        return Decision(action=action, confidence=conf, directional_bias=bias, components=comps)
