from __future__ import annotations

from dataclasses import replace
from typing import Dict, Any, List, Tuple
import numpy as np

from .config import CONFIG
from .utils import log, clamp, STATE
from .portfolio import Target
from .perps_risk import enforce_perps_governor


def _get_closes(bundle: Dict[str, Any], sym: str, tf: str) -> np.ndarray:
    row = (bundle.get("market", {}) or {}).get(sym, {}) or {}
    tfs = ((row.get("index", {}) or {}).get("tfs", {}) or {})
    arr = tfs.get(tf)
    if arr is None:
        return np.array([], dtype=np.float64)
    try:
        a = np.asarray(arr, dtype=np.float64)
        if a.ndim != 2 or a.shape[1] < 5:
            return np.array([], dtype=np.float64)
        return a[:, 4].astype(np.float64)  # close
    except Exception:
        return np.array([], dtype=np.float64)


def _corr_to_btc(bundle: Dict[str, Any], sym: str, tf: str) -> float:
    btc_sym = str(CONFIG.get("BTC_REFERENCE_SYMBOL", "BTC/USDT"))
    if sym == btc_sym:
        return 1.0
    c1 = _get_closes(bundle, sym, tf)
    c2 = _get_closes(bundle, btc_sym, tf)
    if len(c1) < 60 or len(c2) < 60:
        return 0.0
    n = min(len(c1), len(c2))
    r1 = np.diff(np.log(c1[-n:]))
    r2 = np.diff(np.log(c2[-n:]))
    if len(r1) < 30:
        return 0.0
    try:
        return float(np.corrcoef(r1, r2)[0, 1])
    except Exception:
        return 0.0


def enforce_risk(targets: List[Target], prices: Dict[Tuple[str, str], float], bundle: Dict[str, Any], equity: float) -> List[Target]:
    """Clamp targets to hard institutional-style risk rails.

    This function is intentionally conservative and deterministic.
    """
    if not targets:
        return targets

    eq = float(equity) if equity and equity > 0 else float(CONFIG.get("STARTING_EQUITY", 0.0))
    if eq <= 0:
        return targets

    max_gross = float(CONFIG.get("MAX_GROSS_EXPOSURE_PCT", 0.70))
    max_net = float(CONFIG.get("MAX_NET_EXPOSURE_PCT", 0.35))
    per_asset = float(CONFIG.get("PER_ASSET_MAX_EXPOSURE_PCT", 0.15))

    # Per-venue caps (optional)
    venue_caps = CONFIG.get("PER_VENUE_MAX_EXPOSURE_PCT", {
        "COINBASE": 0.35,
        "KRAKEN": 0.35,
        "BINANCEUS": 0.35,
        "BTCC_SPOT": 0.35,
        "BTCC_PERP": 0.35,
    })

    # Step 1: clamp per-asset and per-venue notional
    out: List[Target] = []
    venue_notional: Dict[str, float] = {}
    asset_notional: Dict[str, float] = {}

    for t in targets:
        key = (t.symbol, t.venue)
        p = float(prices.get(key, 0.0))
        if p <= 0:
            continue
        notion = abs(t.qty) * p

        asset_cap = per_asset * eq
        if notion > asset_cap:
            scale = asset_cap / max(notion, 1e-12)
            t = replace(t, qty=t.qty * scale)
            notion = abs(t.qty) * p

        vcap = float((venue_caps or {}).get(t.venue, 1.0)) * eq
        vprev = venue_notional.get(t.venue, 0.0)
        if vprev + notion > vcap:
            remaining = max(0.0, vcap - vprev)
            scale = remaining / max(notion, 1e-12)
            t = replace(t, qty=t.qty * scale)
            notion = abs(t.qty) * p

        venue_notional[t.venue] = vprev + notion
        asset_notional[t.symbol] = asset_notional.get(t.symbol, 0.0) + notion
        out.append(t)

    targets = out
    if not targets:
        return targets

    # Step 2: correlation concentration cap (approx by BTC correlation buckets)
    tf = str(CONFIG.get("CORR_TF", "1h"))
    corr_thr = float(CONFIG.get("BTC_CORR_THRESHOLD", 0.75))
    corr_bucket_cap = float(CONFIG.get("BTC_CORR_BUCKET_CAP_PCT", 0.45)) * eq

    bucket_notion = 0.0
    bucket_targets_idx = []
    for i, t in enumerate(targets):
        p = float(prices.get((t.symbol, t.venue), 0.0))
        if p <= 0:
            continue
        c = _corr_to_btc(bundle, t.symbol, tf)
        if abs(c) >= corr_thr:
            bucket_notion += abs(t.qty) * p
            bucket_targets_idx.append(i)

    if bucket_targets_idx and bucket_notion > corr_bucket_cap:
        scale = corr_bucket_cap / max(bucket_notion, 1e-12)
        log.warning(f"[risk] BTC-corr bucket capped scale={scale:.3f}")
        new = []
        for i, t in enumerate(targets):
            if i in bucket_targets_idx:
                new.append(replace(t, qty=t.qty * scale))
            else:
                new.append(t)
        targets = new

    # Step 3: gross/net portfolio caps
    gross = 0.0
    net = 0.0
    for t in targets:
        p = float(prices.get((t.symbol, t.venue), 0.0))
        gross += abs(t.qty) * p
        net += t.qty * p

    gross_cap = max_gross * eq
    if gross > gross_cap:
        scale = gross_cap / max(gross, 1e-12)
        log.warning(f"[risk] gross exposure capped scale={scale:.3f}")
        targets = [replace(t, qty=t.qty * scale) for t in targets]
        gross = gross_cap
        net *= scale

    net_cap = max_net * eq
    if abs(net) > net_cap:
        scale = net_cap / max(abs(net), 1e-12)
        log.warning(f"[risk] net exposure capped scale={scale:.3f}")
        targets = [replace(t, qty=t.qty * scale) for t in targets]

    return targets
