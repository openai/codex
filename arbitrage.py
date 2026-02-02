from __future__ import annotations
from typing import Dict, Any, List

def detect_simple_arbitrage(prices: Dict[str,float], threshold_bps: float = 15.0) -> List[Dict[str,Any]]:
    keys = list(prices.keys())
    out = []
    for i in range(len(keys)):
        for j in range(i+1, len(keys)):
            a,b = keys[i], keys[j]
            pa,pb = float(prices[a]), float(prices[b])
            if pa<=0 or pb<=0: continue
            spread_bps = (pb/pa - 1.0) * 10000.0
            if abs(spread_bps) >= threshold_bps:
                out.append({"buy": a if spread_bps>0 else b, "sell": b if spread_bps>0 else a, "spread_bps": spread_bps})
    return out
