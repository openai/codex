from __future__ import annotations
import threading
from typing import Any, Dict
from flask import Flask, request, jsonify
from .config import CONFIG
from .db import memorize
from .utils import log


def tv_bias_conf(symbol: str, tv_data: Dict[str, Any]) -> tuple[float, float]:
    """Convert TradingView data into a lightweight bias/conf pair.

    Works with the dictionary produced by `tradingview-ta` in data_sources.
    """
    screener = (tv_data or {}).get("screener", {}) or {}
    rec = str(screener.get("RECOMMENDATION", "")).upper()
    # Bias: STRONG_BUY/BUY vs STRONG_SELL/SELL
    if rec in ("STRONG_BUY", "BUY"):
        bias = 1.0 if rec == "STRONG_BUY" else 0.6
        conf = 0.75 if rec == "STRONG_BUY" else 0.60
        return float(bias), float(conf)
    if rec in ("STRONG_SELL", "SELL"):
        bias = -1.0 if rec == "STRONG_SELL" else -0.6
        conf = 0.75 if rec == "STRONG_SELL" else 0.60
        return float(bias), float(conf)
    if rec in ("NEUTRAL", ""):
        return 0.0, 0.0
    # Unknown label -> be conservative
    return 0.0, 0.0

def start_tv_webhook_server() -> None:
    port = int(CONFIG.get("TRADINGVIEW_WEBHOOK_PORT", 5000))
    path = str(CONFIG.get("TRADINGVIEW_WEBHOOK_PATH", "/tv_webhook"))
    app = Flask("aether_edge_tv")

    @app.route(path, methods=["POST"])
    def tv_webhook():
        alert: Dict[str, Any] = request.get_json(force=True, silent=True) or {}
        memorize("tv_alert", alert)
        log.info(f"[TV] alert saved keys={list(alert.keys())}")
        return jsonify({"ok": True}), 200

    threading.Thread(target=app.run, kwargs={"host":"0.0.0.0","port":port}, daemon=True).start()
    log.info(f"[TV] listening on http://0.0.0.0:{port}{path}")
