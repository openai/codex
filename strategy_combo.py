from __future__ import annotations

"""Combined strategy engine: Trend/Momentum (#3) + Carry/Funding (#2).

Design goals based on our dialogue:
 - Trend/momentum is the directional sleeve (spot by default; shorts via BTCC perps if enabled).
 - Carry is a delta-neutral sleeve (spot + perp) primarily executed on BTCC to avoid transfers.
 - TradingView Ultimate is treated as a *state/event* feed (webhook alerts) and as a secondary TA bias.
 - A risk allocator shifts budget between sleeves using trend strength and volatility.
 - Everything is gated by hard risk rails from config.
"""

from dataclasses import dataclass
from typing import Any, Dict, List, Optional
import math
import numpy as np

from .config import CONFIG
from .utils import clamp
from .tv_webhook import tv_bias_conf


def _log_returns(close: np.ndarray) -> np.ndarray:
    c = np.maximum(close.astype(np.float64), 1e-12)
    return np.diff(np.log(c))


def _ewma_vol(r: np.ndarray, span: int) -> float:
    if r.size < 5:
        return 0.0
    span = max(5, int(span))
    alpha = 2.0 / (span + 1.0)
    v = float(np.var(r[-span:]))
    for x in r[-span:]:
        v = (1 - alpha) * v + alpha * float(x * x)
    return float(math.sqrt(max(v, 0.0)))


@dataclass
class TrendSignal:
    side: int                 # -1,0,+1
    confidence: float         # 0..1
    trend_strength: float     # TS = |ret|/vol
    vol: float                # per-bar vol (log-return std proxy)


class TrendEngine:
    """Multi-horizon TSMOM ensemble + basic breakout confirmation.

    Uses OHLCV from `bundle["market"][symbol]["index"]["tfs"][tf]`.
    """

    def __init__(self):
        self.lookbacks = CONFIG.get("TREND_LOOKBACKS", [48, 144, 480])  # in bars of the primary TF
        self.weights = CONFIG.get("TREND_WEIGHTS", [0.35, 0.40, 0.25])
        self.vol_span = int(CONFIG.get("TREND_VOL_SPAN", 120))
        self.ts_chop = float(CONFIG.get("TREND_TS_CHOP", 0.6))
        self.ts_trend = float(CONFIG.get("TREND_TS_TREND", 1.2))
        self.breakout_len = int(CONFIG.get("TREND_BREAKOUT_LEN", 55))
        self.require_breakout = bool(CONFIG.get("TREND_REQUIRE_BREAKOUT", False))

    def compute(self, ohlcv: np.ndarray) -> TrendSignal:
        if not isinstance(ohlcv, np.ndarray) or ohlcv.shape[0] < 60:
            return TrendSignal(side=0, confidence=0.0, trend_strength=0.0, vol=0.0)

        close = ohlcv[:, 4]
        r = _log_returns(close)
        vol = _ewma_vol(r, self.vol_span)
        if vol <= 0:
            return TrendSignal(side=0, confidence=0.0, trend_strength=0.0, vol=0.0)

        # Multi-horizon momentum votes
        bias = 0.0
        conf = 0.0
        for L, w in zip(self.lookbacks, self.weights):
            L = int(L)
            if close.shape[0] <= L + 1:
                continue
            ret = math.log(max(close[-1], 1e-12) / max(close[-1 - L], 1e-12))
            s = 1.0 if ret > 0 else (-1.0 if ret < 0 else 0.0)
            bias += float(w) * s
            # confidence increases with normalized return magnitude
            ts = abs(ret) / max(vol * math.sqrt(L), 1e-12)
            conf += float(w) * clamp(ts / 2.0, 0.0, 1.0)

        bias = clamp(bias, -1.0, 1.0)

        # Breakout confirmation (optional)
        brk_ok = True
        if self.breakout_len > 10 and close.shape[0] > self.breakout_len + 1:
            hi = float(np.max(close[-1 - self.breakout_len : -1]))
            lo = float(np.min(close[-1 - self.breakout_len : -1]))
            if bias > 0:
                brk_ok = float(close[-1]) >= hi
            elif bias < 0:
                brk_ok = float(close[-1]) <= lo

        # Trend strength on a medium horizon
        Lm = int(self.lookbacks[min(1, len(self.lookbacks) - 1)])
        if close.shape[0] > Lm + 1:
            retm = math.log(max(close[-1], 1e-12) / max(close[-1 - Lm], 1e-12))
            ts = abs(retm) / max(vol * math.sqrt(Lm), 1e-12)
        else:
            ts = 0.0

        # Chop gate
        if ts < self.ts_chop:
            side = 0
        else:
            side = 1 if bias > 0 else (-1 if bias < 0 else 0)

        if self.require_breakout and not brk_ok:
            side = 0

        # Confidence blends magnitude + chop/trend state
        conf = clamp(0.25 + 0.55 * conf + 0.20 * clamp((ts - self.ts_chop) / max(1e-9, (self.ts_trend - self.ts_chop)), 0.0, 1.0), 0.0, 1.0)

        return TrendSignal(side=side, confidence=conf, trend_strength=float(ts), vol=float(vol))


@dataclass
class CarrySignal:
    enabled: bool
    direction: int            # +1 means long spot / short perp (positive funding harvest)
    expected_net: float       # expected net carry per interval (rough)
    funding: float
    basis: float


class CarryEngine:
    """Funding/basis carry logic for perps.

    Default: only run long-spot/short-perp when funding is positive above threshold.
    """

    def __init__(self):
        self.funding_min = float(CONFIG.get("CARRY_FUNDING_MIN", 0.0001))  # 1bp per interval
        self.ts_disable = float(CONFIG.get("CARRY_DISABLE_TS", 2.0))
        self.basis_buffer = float(CONFIG.get("CARRY_BASIS_BUFFER", 0.002))
        self.cost_buffer = float(CONFIG.get("CARRY_COST_BUFFER", 0.001))

    def compute(
        self,
        spot_price: float,
        perp_price: float,
        funding: float,
        trend_strength: float,
    ) -> CarrySignal:
        if spot_price <= 0 or perp_price <= 0:
            return CarrySignal(False, 0, 0.0, float(funding), 0.0)

        # log basis
        basis = math.log(perp_price / spot_price)

        # Disable carry in extreme trend regimes (basis can blow out)
        if trend_strength >= self.ts_disable:
            return CarrySignal(False, 0, 0.0, float(funding), float(basis))

        # Only harvest positive funding by default
        if funding < self.funding_min:
            return CarrySignal(False, 0, 0.0, float(funding), float(basis))

        # Expected net: funding - buffers
        expected_net = float(funding) - self.cost_buffer - self.basis_buffer * abs(basis)
        enabled = expected_net > 0.0
        return CarrySignal(enabled, +1 if enabled else 0, float(expected_net), float(funding), float(basis))


class ComboEngine:
    """Portfolio-level decisioning.

    Outputs a list of executable order intents:
      {symbol, side, units, price, venue, sleeve, confidence, meta...}
    """

    def __init__(self):
        self.trend = TrendEngine()
        self.carry = CarryEngine()

    def _target_notional(self, equity: float, conf: float, sleeve: str) -> float:
        # Sleeve budgets as fractions of equity; allocator adjusts them.
        if sleeve == "trend":
            base = float(CONFIG.get("TREND_RISK_PCT", 0.03))
        else:
            base = float(CONFIG.get("CARRY_RISK_PCT", 0.02))
        return float(equity) * base * (0.5 + 0.5 * clamp(conf, 0.0, 1.0))

    def decide_portfolio(self, symbols: List[str], bundle: Dict[str, Any], equity: float) -> List[Dict[str, Any]]:
        market = bundle.get("market", {}) or {}
        out: List[Dict[str, Any]] = []

        primary_tf = str(CONFIG.get("TREND_PRIMARY_TF", "1h"))
        allow_short = bool(CONFIG.get("ALLOW_SHORT", True))
        trend_vs_carry_tau = float(CONFIG.get("ALLOC_TS_TAU", 1.2))
        trend_weight_hi = float(CONFIG.get("ALLOC_TREND_WEIGHT_HI", 0.70))
        trend_weight_lo = float(CONFIG.get("ALLOC_TREND_WEIGHT_LO", 0.35))

        # Focus carry on top symbols only (configurable)
        carry_syms = set(CONFIG.get("CARRY_SYMBOLS", CONFIG.get("SYMBOLS", [])))

        for sym in symbols:
            row = market.get(sym, {}) or {}
            idx = (row.get("index", {}) or {})
            ohlcv = (idx.get("tfs", {}) or {}).get(primary_tf)
            price = float(idx.get("price", 0.0))
            if price <= 0 or not isinstance(ohlcv, np.ndarray):
                continue

            tsig = self.trend.compute(ohlcv)

            # TradingView (Ultimate) treated as a secondary bias/confidence feed
            tv_row = (bundle.get("tv_data", {}) or {}).get(sym, {})
            tv_bias, tv_conf = tv_bias_conf(sym, tv_row)
            tv_blend = float(CONFIG.get("TREND_TV_BLEND", 0.20))
            if tv_bias != 0.0 and tv_conf > 0:
                # If model is flat but TV has conviction, allow a small nudge into the direction.
                if tsig.side == 0:
                    tsig = TrendSignal(
                        side=1 if tv_bias > 0 else -1,
                        confidence=max(tsig.confidence, tv_conf * tv_blend),
                        trend_strength=tsig.trend_strength,
                        vol=tsig.vol
                    )
                else:
                    # If they agree, slightly increase confidence; if they disagree, slightly decrease.
                    agree = (tsig.side > 0 and tv_bias > 0) or (tsig.side < 0 and tv_bias < 0)
                    if agree:
                        tsig = TrendSignal(
                            side=tsig.side,
                            confidence=clamp(tsig.confidence + tv_conf * tv_blend, 0.0, 1.0),
                            trend_strength=tsig.trend_strength,
                            vol=tsig.vol
                        )
                    else:
                        tsig = TrendSignal(
                            side=tsig.side,
                            confidence=clamp(tsig.confidence * (1.0 - 0.5 * tv_blend), 0.0, 1.0),
                            trend_strength=tsig.trend_strength,
                            vol=tsig.vol
                        )

# Allocator: when trend strength is high, shift budget to trend and away from carry
            trend_w = trend_weight_hi if tsig.trend_strength >= trend_vs_carry_tau else trend_weight_lo
            carry_w = 1.0 - trend_w

            # TREND sleeve decision
            if tsig.side != 0:
                side = "LONG" if tsig.side > 0 else "SHORT"
                if side == "SHORT" and not allow_short:
                    side = "FLAT"
                if side != "FLAT":
                    notional = self._target_notional(equity, tsig.confidence, "trend") * trend_w
                    units = notional / max(price, 1e-12)
                    out.append(
                        {
                            "symbol": sym,
                            "side": side,
                            "units": float(units),
                            "price": float(price),
                            "venue": "AUTO_SPOT" if side == "LONG" else "AUTO_SHORT",
                            "sleeve": "trend",
                            "confidence": float(tsig.confidence),
                            "trend_strength": float(tsig.trend_strength),
                            "vol": float(tsig.vol),
                        }
                    )

            # CARRY sleeve decision (only for selected symbols)
            if sym in carry_syms and bool(CONFIG.get("CARRY_ENABLED", True)):
                perp = (row.get("btcc_perp", {}) or {})
                spot_btcc = (row.get("btcc_spot", {}) or {})
                spot_price = float(spot_btcc.get("price", price))  # default to index if missing
                perp_price = float(perp.get("price", 0.0))
                funding = float(perp.get("funding", 0.0))
                csig = self.carry.compute(spot_price=spot_price, perp_price=perp_price, funding=funding, trend_strength=tsig.trend_strength)
                if csig.enabled and csig.direction == 1:
                    notional = self._target_notional(equity, clamp(csig.expected_net / max(1e-9, self.carry.funding_min), 0.0, 1.0), "carry") * carry_w
                    units = notional / max(spot_price, 1e-12)
                    # Carry requires two legs: long spot + short perp
                    out.append(
                        {
                            "symbol": sym,
                            "side": "LONG",
                            "units": float(units),
                            "price": float(spot_price),
                            "venue": "BTCC_SPOT",
                            "sleeve": "carry",
                            "confidence": float(clamp(csig.expected_net / 0.001, 0.0, 1.0)),
                            "funding": csig.funding,
                            "basis": csig.basis,
                            "expected_net": csig.expected_net,
                        }
                    )
                    out.append(
                        {
                            "symbol": sym,
                            "side": "SHORT",
                            "units": float(units),
                            "price": float(perp_price),
                            "venue": "BTCC_PERP",
                            "sleeve": "carry",
                            "confidence": float(clamp(csig.expected_net / 0.001, 0.0, 1.0)),
                            "funding": csig.funding,
                            "basis": csig.basis,
                            "expected_net": csig.expected_net,
                        }
                    )

        # Rebalance band / cooldown handled by broker; caller can also trim here if desired.
        return out
