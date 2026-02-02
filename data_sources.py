from __future__ import annotations
from typing import Dict, Any
import asyncio
import numpy as np
from .config import CONFIG
from .utils import log
from .db import memorize, store_candles
from .sentiment import fetch_rss_sentiment

try:
    import ccxt.async_support as ccxt_async  # type: ignore
except Exception:
    ccxt_async = None  # type: ignore

try:
    from tradingview_ta import TA_Handler, Interval  # type: ignore
except Exception:
    TA_Handler = None  # type: ignore
    Interval = None  # type: ignore

def _tv_interval():
    if Interval is None: return None
    m = str(CONFIG.get("TRADINGVIEW_INTERVAL", "5m")).lower()
    return {
        "1m": Interval.INTERVAL_1_MINUTE,
        "5m": Interval.INTERVAL_5_MINUTES,
        "15m": Interval.INTERVAL_15_MINUTES,
        "1h": Interval.INTERVAL_1_HOUR,
        "4h": Interval.INTERVAL_4_HOURS,
        "1d": Interval.INTERVAL_1_DAY,
    }.get(m, Interval.INTERVAL_5_MINUTES)

async def fetch_tv_data(symbol: str) -> Dict[str, Any]:
    if not bool(CONFIG.get("TRADINGVIEW_ENABLED", True)) or TA_Handler is None:
        return {"indicators": {}, "patterns": {}, "screener": {}}
    exch = str(CONFIG.get("TRADINGVIEW_EXCHANGE", "BINANCE"))
    interval = _tv_interval()
    sym = symbol.replace("/", "")

    def call():
        h = TA_Handler(symbol=sym, screener="crypto", exchange=exch, interval=interval)
        a = h.get_analysis()
        return {"indicators": dict(a.indicators or {}), "patterns": dict(a.oscillators or {}), "screener": dict(a.summary or {})}

    try:
        return await asyncio.to_thread(call)
    except Exception as e:
        log.debug(f"TV failed {symbol}: {e}")
        return {"indicators": {}, "patterns": {}, "screener": {}}

async def _fetch_one_exchange(ex_id: str, symbols: list[str], tfs: list[str], limit: int) -> Dict[str, Any]:
    """Fetch tickers + OHLCV for a single exchange via ccxt.

    Returns {symbol: {price, bid, ask, tfs{tf: np.ndarray}}}
    """
    out: Dict[str, Any] = {}
    ex = getattr(ccxt_async, ex_id)()
    try:
        tickers = await asyncio.gather(*[ex.fetch_ticker(s) for s in symbols], return_exceptions=True)
        for s, t in zip(symbols, tickers):
            if isinstance(t, Exception):
                continue
            last = float((t.get("last") or t.get("close") or 0.0))
            bid = float(t.get("bid") or 0.0)
            ask = float(t.get("ask") or 0.0)
            out[s] = {"price": last, "bid": bid, "ask": ask, "tfs": {}}

        for s in symbols:
            for tf in tfs:
                try:
                    ohlcv = await ex.fetch_ohlcv(s, timeframe=tf, limit=limit)
                    if ohlcv:
                        store_candles(s, tf, ohlcv)
                        out.setdefault(s, {"price": 0.0, "bid": 0.0, "ask": 0.0, "tfs": {}})["tfs"][tf] = np.array(ohlcv, dtype=np.float64)
                except Exception as e:
                    log.debug(f"ohlcv failed {ex_id} {s} {tf}: {e}")
    finally:
        try:
            await ex.close()
        except Exception:
            pass
    return out


async def fetch_exchange_data() -> Dict[str, Any]:
    """Fetch market data for multiple spot venues + BTCC perps.

    Output schema (per symbol):
      {
        "venues": {"COINBASE": {...}, "KRAKEN": {...}, "BINANCEUS": {...}},
        "index":  {"price": median_price, "tfs": {...}},
        "btcc_spot": {...},
        "btcc_perp": {...}
      }
    """
    symbols = list(CONFIG.get("SYMBOLS", []))
    tfs = list(CONFIG.get("TIMEFRAMES", ["5m"]))
    limit = int(CONFIG.get("OHLCV_LIMIT", 500))
    out: Dict[str, Any] = {}

    if ccxt_async is None:
        # Synthetic schema-compatible data (offline/dev)
        for s in symbols:
            tfs_map = {}
            for tf in tfs:
                tfs_map[tf] = np.random.rand(min(200, limit), 6)
            out[s] = {
                "venues": {"PAPER": {"price": 0.0, "bid": 0.0, "ask": 0.0, "tfs": tfs_map}},
                "index": {"price": 0.0, "tfs": tfs_map},
                "btcc_spot": {},
                "btcc_perp": {"price": 0.0, "funding": 0.0},
            }
        return out

    # Live ccxt path
    venues = {
        "COINBASE": str(CONFIG.get("COINBASE_EXCHANGE_ID", "coinbase")),
        "KRAKEN": str(CONFIG.get("KRAKEN_EXCHANGE_ID", "kraken")),
        "BINANCEUS": str(CONFIG.get("BINANCEUS_EXCHANGE_ID", "binanceus")),
        "BTCC": str(CONFIG.get("BTCC_EXCHANGE_ID", "btcc")),
    }

    spot_venue_list = [v for v in ("COINBASE", "KRAKEN", "BINANCEUS") if bool(CONFIG.get(f"{v}_ENABLED", True))]

    spot_results = await asyncio.gather(
        *[_fetch_one_exchange(venues[v], symbols, tfs, limit) for v in spot_venue_list],
        return_exceptions=True,
    )

    # BTCC spot + perps: we fetch tickers; funding may not be supported on all ccxt builds.
    btcc_spot = {}
    btcc_perp = {}
    try:
        btcc_data = await _fetch_one_exchange(venues["BTCC"], symbols, tfs, limit)
        btcc_spot = btcc_data
        # Try funding rates via ccxt if available
        ex = getattr(ccxt_async, venues["BTCC"])()
        try:
            for s in symbols:
                fr = 0.0
                try:
                    if hasattr(ex, "fetch_funding_rate"):
                        r = await ex.fetch_funding_rate(s)
                        fr = float((r or {}).get("fundingRate") or 0.0)
                    elif hasattr(ex, "fetch_funding_rates"):
                        r = await ex.fetch_funding_rates([s])
                        fr = float(((r or {}).get(s) or {}).get("fundingRate") or 0.0)
                except Exception:
                    fr = 0.0
                # Treat btcc_spot ticker price as spot; for perp price use same unless distinct markets are configured.
                p = float((btcc_data.get(s, {}) or {}).get("price") or 0.0)
                btcc_perp[s] = {"price": p, "funding": fr}
        finally:
            try:
                await ex.close()
            except Exception:
                pass
    except Exception as e:
        log.debug(f"BTCC fetch failed: {e}")

    # Assemble per-symbol structure
    for s in symbols:
        venues_map: Dict[str, Any] = {}
        prices: List[float] = []
        tfs_map: Dict[str, Any] = {}

        for v, res in zip(spot_venue_list, spot_results):
            if isinstance(res, Exception):
                continue
            r = (res or {}).get(s)
            if not r:
                continue
            venues_map[v] = r
            p = float(r.get("price") or 0.0)
            if p > 0:
                prices.append(p)
            # Use the first venue providing OHLCV as the index candle source (can be upgraded later).
            if not tfs_map and isinstance(r.get("tfs"), dict):
                tfs_map = r.get("tfs")

        # Index price = median of venue prices
        idx_price = float(np.median(np.array(prices))) if prices else 0.0

        out[s] = {
            "venues": venues_map,
            "index": {"price": idx_price, "tfs": tfs_map},
            "btcc_spot": btcc_spot.get(s, {}),
            "btcc_perp": btcc_perp.get(s, {}),
        }

    return out

async def fetch_all() -> Dict[str, Any]:
    market = await fetch_exchange_data()
    tv_data: Dict[str, Any] = {}
    for sym in list(CONFIG.get("SYMBOLS", [])):
        tv_data[sym] = await fetch_tv_data(sym)
    memorize("tv_data", tv_data)
    sent = await fetch_rss_sentiment()
    return {"market": market, "tv_data": tv_data, "sentiment": sent}
