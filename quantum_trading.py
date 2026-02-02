from __future__ import annotations
from typing import Dict, Tuple, List
import numpy as np

def select_symbols_qubo(scores: Dict[str, float], corr: Dict[Tuple[str,str], float], k: int = 2,
                        lam: float = 0.8, steps: int = 1200, seed: int = 7) -> List[str]:
    rng = np.random.default_rng(seed)
    syms = list(scores.keys())
    n = len(syms)
    if n <= k: return syms
    x = np.zeros(n, dtype=np.int8)
    x[rng.choice(n, size=k, replace=False)] = 1

    def energy(xv):
        e = 0.0
        e -= sum(scores[syms[i]] * xv[i] for i in range(n))
        for i in range(n):
            if xv[i]==0: continue
            for j in range(i+1,n):
                if xv[j]==0: continue
                cij = corr.get((syms[i],syms[j]), corr.get((syms[j],syms[i]), 0.0))
                e += lam * float(cij)
        return e

    best = x.copy(); best_e = energy(x)
    T0 = 1.0
    for t in range(steps):
        T = max(1e-3, T0*(1 - t/steps))
        i = int(rng.integers(0,n))
        x2 = x.copy(); x2[i] = 1 - x2[i]
        while x2.sum() > k:
            ones = np.where(x2==1)[0]; x2[int(rng.choice(ones))]=0
        while x2.sum() < k:
            zeros = np.where(x2==0)[0]; x2[int(rng.choice(zeros))]=1
        e1 = energy(x); e2 = energy(x2)
        de = e2-e1
        if de < 0 or rng.random() < np.exp(-de/T):
            x = x2
        if e2 < best_e:
            best_e = e2; best = x2
    return [syms[i] for i in range(n) if best[i]==1]
