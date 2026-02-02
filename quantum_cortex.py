from __future__ import annotations

import time
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Tuple

import numpy as np

from .utils import clamp, log

try:
    from sklearn.cluster import KMeans  # type: ignore
except Exception:  # pragma: no cover
    KMeans = None  # type: ignore


@dataclass
class QCortexOut:
    phase: str                  # markup | distribution | chop
    score: float                # -1..1 (directional)
    confidence: float           # 0..1
    fd: float                   # fractal dimension proxy
    vol: float                  # realized vol proxy (0..1)
    hawkes: float               # intensity proxy (0..1)
    entropy: float              # normalized entropy proxy (0..1)


class QuantumCortex:
    """Quantitative Cortex (market regime / complexity guard).

    This is NOT quantum computing; it's the bot's *quant* brain:
    - Higuchi-like fractal dimension (complexity / trendiness)
    - Realized-volatility proxy
    - Hawkes-style event intensity proxy (from candle timestamps)
    - Entropy guard on returns (avoid noisy regimes)
    - Regime clustering (markup / distribution / chop)

    The output is a directional score (-1..1) plus a confidence gate.
    """

    def __init__(self, n_clusters: int = 3):
        self.n_clusters = int(n_clusters)
        self.regimes = None
        self._last_fit_ts = 0.0

    # ---------- Feature extraction ----------
    def _returns(self, prices: np.ndarray) -> np.ndarray:
        prices = np.asarray(prices, dtype=np.float64)
        prices = np.maximum(prices, 1e-12)
        r = np.diff(np.log(prices))
        return r[np.isfinite(r)]

    def realized_vol(self, prices: List[float]) -> float:
        r = self._returns(np.array(prices))
        if r.size < 10:
            return 0.0
        rv = float(np.std(r) * np.sqrt(252.0))  # annualized-ish
        # squish to 0..1 for downstream weighting
        return float(clamp(rv / 1.0, 0.0, 1.0))

    def higuchi_fd(self, prices: List[float]) -> float:
        """Robust Higuchi fractal dimension (small-k), returns ~[1,2]."""
        x = np.asarray(prices, dtype=np.float64)
        n = x.size
        if n < 64:
            return 1.5
        max_k = min(20, n // 5)
        L = []
        k_values = range(2, max_k + 1)
        for k in k_values:
            Lk = 0.0
            for m in range(k):
                idx = np.arange(m, n, k)
                if idx.size < 2:
                    continue
                diffs = np.abs(np.diff(x[idx]))
                Lm = np.sum(diffs) * (n - 1) / (idx.size * k)
                Lk += Lm
            Lk /= max(1, k)
            if Lk > 0:
                L.append((k, Lk))
        if len(L) < 6:
            return 1.5
        ks = np.log([p[0] for p in L])
        Ls = np.log([p[1] for p in L])
        # slope of log(L(k)) vs log(1/k) => FD
        slope = np.polyfit(ks, Ls, 1)[0]
        fd = 2.0 - float(slope)
        return float(clamp(fd, 1.0, 2.0))

    def hawkes_intensity(self, times: List[float]) -> float:
        """Simple exponential-kernel intensity proxy. Input times are seconds or ms."""
        if not times or len(times) < 16:
            return 0.5
        t = np.asarray(times[-200:], dtype=np.float64)
        if t.max() > 1e12:  # ms -> s
            t = t / 1000.0
        t = t - t.min()
        # exponential kernel
        alpha = 0.30
        beta = 0.25
        lam = 0.5
        for i in range(1, len(t)):
            dt = max(1e-6, float(t[i] - t[i-1]))
            lam = 0.5 + alpha * np.exp(-beta * dt) + 0.85 * (lam - 0.5)
        return float(clamp(lam, 0.0, 1.0))

    def entropy_guard(self, prices: List[float]) -> Tuple[bool, float]:
        r = self._returns(np.array(prices))
        if r.size < 40:
            return True, 0.0
        # histogram entropy (normalized)
        hist, _ = np.histogram(r, bins=20, density=True)
        hist = hist + 1e-12
        H = float(-np.sum(hist * np.log(hist)))
        Hn = float(clamp(H / 6.0, 0.0, 1.0))
        # Lower entropy tends to mean more structure
        return (Hn < 0.72), Hn

    # ---------- Regime inference ----------
    def _fit_regimes(self, feats: np.ndarray) -> None:
        if KMeans is None:
            return
        try:
            self.regimes = KMeans(n_clusters=self.n_clusters, n_init=10, random_state=7).fit(feats)
            self._last_fit_ts = time.time()
        except Exception:
            self.regimes = None

    def classify_phase(self, feat: np.ndarray) -> str:
        # If clustering unavailable, use heuristic on fd/vol
        fd, vol, hawkes = float(feat[0]), float(feat[1]), float(feat[2])
        if self.regimes is None:
            if vol > 0.75:
                return "chop"
            return "markup" if fd < 1.45 else "distribution" if fd > 1.65 else "chop"

        label = int(self.regimes.predict(feat.reshape(1, -1))[0])
        # Map cluster -> phase by ordering on fd (low fd ~ trending)
        centers = np.asarray(self.regimes.cluster_centers_, dtype=np.float64)
        order = np.argsort(centers[:, 0])  # sort by fd
        # lowest fd => markup, highest fd => distribution, middle => chop
        if label == int(order[0]):
            return "markup"
        if label == int(order[-1]):
            return "distribution"
        return "chop"

    # ---------- Public API ----------
    def analyze(self, symbol: str, market: Dict[str, Any]) -> Dict[str, Any]:
        sym = market.get(symbol, {}) if isinstance(market, dict) else {}
        tfs = (sym.get("tfs") or {}) if isinstance(sym, dict) else {}
        # pick the smallest timeframe available
        arr = None
        if isinstance(tfs, dict) and tfs:
            # try common order
            for tf in ("1m", "3m", "5m", "15m", "1h"):
                if tf in tfs:
                    arr = tfs[tf]
                    break
            if arr is None:
                arr = next(iter(tfs.values()))
        if arr is None:
            return {"phase": "chop", "score": 0.0, "confidence": 0.0, "reason": "no_ohlcv"}

        try:
            a = np.asarray(arr, dtype=np.float64)
            closes = a[:, 4].tolist()
            times = a[:, 0].tolist()
        except Exception:
            return {"phase": "chop", "score": 0.0, "confidence": 0.0, "reason": "bad_ohlcv"}

        if len(closes) < 80:
            return {"phase": "chop", "score": 0.0, "confidence": 0.0, "reason": "short_history"}

        fd = self.higuchi_fd(closes[-400:])
        vol = self.realized_vol(closes[-400:])
        hawkes = self.hawkes_intensity(times[-400:])
        ok, Hn = self.entropy_guard(closes[-400:])

        feat = np.array([fd, vol, hawkes], dtype=np.float64)

        # periodic refit
        if KMeans is not None and (self.regimes is None or (time.time() - self._last_fit_ts) > 3600):
            # build a small rolling feature matrix
            feats = []
            w = 120
            step = 10
            for i in range(w, min(len(closes), 600), step):
                seg = closes[-i:]
                feats.append([
                    self.higuchi_fd(seg[-240:]),
                    self.realized_vol(seg[-240:]),
                    self.hawkes_intensity(times[-240:]),
                ])
            feats = np.asarray(feats, dtype=np.float64)
            if feats.shape[0] >= 12:
                self._fit_regimes(feats)

        phase = self.classify_phase(feat)

        # Directional score: markup=>+1, distribution=>-1, chop=>0
        score = 1.0 if phase == "markup" else (-1.0 if phase == "distribution" else 0.0)

        # Confidence: penalize entropy + very high vol; reward strong regime signal (fd far from mid)
        fd_strength = float(clamp(abs(fd - 1.5) / 0.25, 0.0, 1.0))
        conf = 0.0
        conf += 0.45 * fd_strength
        conf += 0.25 * (1.0 - Hn)
        conf += 0.30 * (1.0 - vol)

        if not ok:
            conf *= 0.35  # quiet mode when entropy high

        conf = float(clamp(conf, 0.0, 1.0))

        out = QCortexOut(phase=phase, score=float(score), confidence=conf, fd=float(fd), vol=float(vol), hawkes=float(hawkes), entropy=float(Hn))
        return out.__dict__

    def decide(self, symbol: str, market: Dict[str, Any], conf_min: float = 0.60) -> Optional[str]:
        out = self.analyze(symbol, market)
        conf = float(out.get("confidence", 0.0))
        score = float(out.get("score", 0.0))
        phase = str(out.get("phase", "chop"))

        if conf < conf_min or score == 0.0:
            return None
        return "LONG" if score > 0 else "SHORT"
