from __future__ import annotations
import numpy as np
from typing import Dict

def rsi(closes: np.ndarray, period: int = 14) -> float:
    if len(closes) < period + 1:
        return 50.0
    d = np.diff(closes)
    up = np.maximum(d, 0.0)
    dn = np.maximum(-d, 0.0)
    ru = np.mean(up[-period:])
    rd = np.mean(dn[-period:]) + 1e-9
    rs = ru/rd
    return float(100.0 - 100.0/(1.0+rs))

def atr(ohlc: np.ndarray, period: int = 14) -> float:
    if len(ohlc) < period + 1:
        return 0.0
    h,l,c = ohlc[:,1], ohlc[:,2], ohlc[:,3]
    prev = np.roll(c, 1)
    tr = np.maximum(h-l, np.maximum(np.abs(h-prev), np.abs(l-prev)))
    return float(np.mean(tr[-period:]))

def fracdiff(series: np.ndarray, d: float = 0.4, thres: float = 1e-5) -> np.ndarray:
    w = [1.0]
    k = 1
    while True:
        wk = -w[-1] * (d - k + 1) / k
        if abs(wk) < thres:
            break
        w.append(wk); k += 1
        if k > 2000: break
    w = np.array(w[::-1], dtype=np.float64)
    out = np.full_like(series, 0.0, dtype=np.float64)
    for i in range(len(series)):
        if i < len(w)-1: continue
        out[i] = float(np.dot(w, series[i-len(w)+1:i+1]))
    return out

def tech_features(ohlcv: np.ndarray) -> Dict[str, float]:
    closes = ohlcv[:,4].astype(np.float64)
    ohlc = ohlcv[:,1:5].astype(np.float64)
    return {
        "rsi": rsi(closes),
        "atr": atr(ohlc),
        "ret_1": float((closes[-1] / max(1e-9, closes[-2])) - 1.0) if len(closes) >= 2 else 0.0,
        "fd": float(fracdiff(closes)[-1]) if len(closes) else 0.0,
    }
