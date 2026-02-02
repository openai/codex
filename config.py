from __future__ import annotations
import os, json
from typing import Any, Dict

CONFIG: Dict[str, Any] = {
    "version": "3.4-modetoggle",
    "voice": "leo",

    # Trade modes (institutional): paper / pilot / live
    # - paper: simulated execution
    # - pilot: real execution with strict caps (must be armed)
    # - live : real execution with full caps (must be armed)
    "TRADE_MODE": "paper",

    # Legacy internal mode used by RouterBroker: paper/shadow/live.
    # We derive this from TRADE_MODE for compatibility.
    "MODE": "paper",

    # Safety: in paper we always run DRY_RUN and disallow real orders.
    "DRY_RUN": True,
    "ENABLE_REAL_ORDERS": False,

    # Apply built-in mode profiles automatically unless you override.
    "APPLY_MODE_PROFILE": True,

    # Dashboard/runtime control: where a mode request is written.
    # The Core will honor requests fail-closed (pilot/live require arming).
    "MODE_REQUEST_PATH": "mode_request.json",

    # Built-in mode profiles. You can override any field in config.json.
    "MODE_PROFILES": {
        "paper": {
            "DRY_RUN": True,
            "ENABLE_REAL_ORDERS": False,
        },
        "pilot": {
            "DRY_RUN": False,
            "ENABLE_REAL_ORDERS": True,
            # Conservative caps: tiny risk
            "MAX_EXPOSURE_PCT": 0.002,
            "MAX_GROSS_EXPOSURE_PCT": 0.10,
            "MAX_NET_EXPOSURE_PCT": 0.06,
            "PER_ASSET_MAX_EXPOSURE_PCT": 0.03,
            "MAX_LEVERAGE_CAP": 1.3,
            "LEVERAGE_MAX": 1.2,
            "DD_PAUSE": 0.01,
            "HIBERNATE_DD": 0.02,
            "MIN_ORDER_NOTIONAL": 10,
            "MAX_TURNOVER_PCT": 0.05,
        },
        "live": {
            "DRY_RUN": False,
            "ENABLE_REAL_ORDERS": True,
        },
    },

    # loop
    "LOOP_SECONDS": 15,
    "COOLDOWN_SEC": 15,

    # UX
    # When starting a live session, prompt interactively for missing API keys (preferred).
    "INTERACTIVE_KEY_PROMPT": True,

    # capital
    "START_FROM_REAL_BALANCE": False,
    "STARTING_EQUITY": 5000.0,

    # risk rails
    "MAX_LEVERAGE_CAP": 3.0,   # HARD CAP
    "LEVERAGE": 1.0,
    "LEVERAGE_MIN": 1.0,
    "LEVERAGE_MAX": 3.0,

    "MAX_EXPOSURE_PCT": 0.05,
    "MAX_EXPOSURE_MIN": 0.01,
    "MAX_EXPOSURE_MAX": 0.08,

    "DD_PAUSE": 0.25,
    "HIBERNATE_DD": 0.30,
    "HIBERNATE_SECONDS": 86400,

    "VOLATILITY_CIRCUIT": 1.8,

    # universe
    "SYMBOLS": ["BTC/USDT", "ETH/USDT"],
    "TOP_N": 2,
    "TIMEFRAMES": ["1m","5m","15m","1h"],
    "OHLCV_LIMIT": 500,

    # exchanges (spot venues) — per our dialogue
    "COINBASE_ENABLED": True,
    "KRAKEN_ENABLED": True,
    "BINANCEUS_ENABLED": True,
    "COINBASE_EXCHANGE_ID": "coinbase",
    "KRAKEN_EXCHANGE_ID": "kraken",
    "BINANCEUS_EXCHANGE_ID": "binanceus",
    "SPOT_ROUTER_DEFAULT": "COINBASE",

    # BTCC venue (spot + perps)
    "BTCC_EXCHANGE_ID": "btcc",
    "BTCC_PERPS_ONLY_FOR_SHORT": True,

    # keys (prefer env; interactive prompts supported)
    "COINBASE_API_KEY": "",
    "COINBASE_API_SECRET": "",
    "KRAKEN_API_KEY": "",
    "KRAKEN_API_SECRET": "",
    "BINANCEUS_API_KEY": "",
    "BINANCEUS_API_SECRET": "",
    "BTCC_API_KEY": "",
    "BTCC_API_SECRET": "",

    # tradingview
    "TRADINGVIEW_ENABLED": True,
    "TRADINGVIEW_EXCHANGE": "BINANCE",
    "TRADINGVIEW_INTERVAL": "5m",
    "TRADINGVIEW_WEBHOOK_PORT": 5000,
    "TRADINGVIEW_WEBHOOK_PATH": "/tv_webhook",
    "TRADINGVIEW_WEBHOOK_URL": "http://localhost:5000/tv_webhook",
    "TRADINGVIEW_SCREENER_FILTERS": {"example":"RSI > 70 AND volume > avg_volume * 1.5"},

    # RSS + sentiment
    "RSS_ENABLED": True,
    "RSS_MIN_SECONDS": 300,
    "RSS_FEEDS": {
        "coindesk": {"url": "https://feeds.feedburner.com/coindesk", "weight": 1.2},
        "bloomberg": {"url": "https://feeds.bloomberg.com/news/rss", "weight": 1.0}
    },

    # AI jury
    "AI_JURY": ["grok", "openai", "claude", "gemini"],
    "GROK_API_KEY": "",
    "OPENAI_API_KEY": "",
    "CLAUDE_API_KEY": "",
    "GEMINI_API_KEY": "",
    "AI_TIMEOUT_SEC": 6,

    # Risk governance
    "RISK_GOV_ENABLED": True,
    "RISK_GOV_INTERVAL_LOOPS": 50,
    "RISK_GOV_COOLDOWN_LOOPS": 50,
    "RISK_GOV_MIN_VOTES": 2,
    "RISK_GOV_APPROVAL": 0.67,
    "BOT_CONF_WEIGHT": 0.45,
    "JUDGES_CONF_WEIGHT": 0.55,

    # fusion weights
    "W_TECH": 0.35,
    "W_SENT": 0.25,
    "W_PATTERN": 0.25,
    "W_TV": 0.15,
    "W_QML": 0.10,

    # entry gates (PolicyEngine)
    "ENTRY_CONF_THRESH": 0.55,
    "ENTRY_BIAS_THRESH": 0.12,

    # ===== Strategy hyperparameters: Trend/Momentum (#3) + Carry (#2) =====
    # Trend
    "TREND_PRIMARY_TF": "1h",
    # lookbacks measured in bars of TREND_PRIMARY_TF
    "TREND_LOOKBACKS": [48, 144, 480],
    "TREND_WEIGHTS": [0.35, 0.40, 0.25],
    "TREND_VOL_SPAN": 120,
    "TREND_TS_CHOP": 0.6,
    "TREND_TS_TREND": 1.2,
    "TREND_BREAKOUT_LEN": 55,
    "TREND_REQUIRE_BREAKOUT": False,
    "TREND_RISK_PCT": 0.03,

    # Carry
    "CARRY_ENABLED": True,
    "CARRY_SYMBOLS": ["BTC/USDT", "ETH/USDT"],
    "CARRY_FUNDING_MIN": 0.0001,
    "CARRY_DISABLE_TS": 2.0,
    "CARRY_BASIS_BUFFER": 0.002,
    "CARRY_COST_BUFFER": 0.001,
    "CARRY_RISK_PCT": 0.02,

    # Allocator
    "ALLOC_TS_TAU": 1.2,
    "ALLOC_TREND_WEIGHT_HI": 0.70,
    "ALLOC_TREND_WEIGHT_LO": 0.35,

    # Shorting
    "ALLOW_SHORT": True,

    # QML
    "ENABLE_QML": True,
    "QML_BACKEND": "default.qubit",
    "QML_N_QUBITS": 4,
    "QML_LAYERS": 2,
    "QML_LR": 0.08,
    "QML_EPOCHS": 25,
    "QML_RETRAIN_EVERY": 200,

    # Optuna live tuning (optional)
    "OPTUNA_ENABLED": False,
    "OPTUNA_EVERY_LOOPS": 300,
    "OPTUNA_TRIALS": 30,
}

def load_config(path: str = "config.json") -> None:
    if not os.path.exists(path):
        return
    try:
        with open(path, "r", encoding="utf-8") as f:
            user = json.load(f)
        for k, v in user.items():
            if isinstance(v, dict) and isinstance(CONFIG.get(k), dict):
                CONFIG[k].update(v)  # type: ignore
            else:
                CONFIG[k] = v
    except Exception:
        pass


def resolve_trade_mode() -> str:
    """Resolve trade mode from env/config (fail-closed to paper)."""
    env_mode = str(os.getenv("AETHER_TRADE_MODE", "") or "").strip().lower()
    cfg_mode = str(CONFIG.get("TRADE_MODE", "") or "").strip().lower()
    legacy = str(CONFIG.get("MODE", "") or "").strip().lower()

    mode = env_mode or cfg_mode
    if not mode:
        # Back-compat mapping: old MODE used paper/shadow/live.
        if legacy in ("paper",):
            mode = "paper"
        elif legacy in ("live",):
            mode = "live"
        else:
            mode = "paper"
    if mode not in ("paper", "pilot", "live"):
        mode = "paper"
    return mode


def apply_mode_profile() -> None:
    """Apply built-in (or user-supplied) mode profile overrides."""
    mode = resolve_trade_mode()
    CONFIG["TRADE_MODE"] = mode

    # RouterBroker compatibility: pilot behaves like live execution.
    CONFIG["MODE"] = "paper" if mode == "paper" else "live"

    if not bool(CONFIG.get("APPLY_MODE_PROFILE", True)):
        return
    profs = CONFIG.get("MODE_PROFILES", {}) or {}
    p = (profs.get(mode) or {}) if isinstance(profs, dict) else {}
    if isinstance(p, dict):
        for k, v in p.items():
            # Do not deep-merge nested dicts here; profiles should be explicit.
            CONFIG[k] = v


def is_armed() -> bool:
    """Return True if real orders are explicitly armed for this trade mode."""
    mode = str(CONFIG.get("TRADE_MODE", "paper")).lower()
    if mode == "paper":
        return False
    if not bool(CONFIG.get("ENABLE_REAL_ORDERS", False)):
        return False
    if bool(CONFIG.get("DRY_RUN", True)):
        return False
    token = "YES_I_UNDERSTAND"
    if mode == "pilot":
        return os.getenv("PILOT_ARMED", "") == token
    return os.getenv("LIVE_ARMED", "") == token


def _mode_request_path() -> str:
    """Path used for mode requests (e.g., dashboard toggle).

    Default is CONFIG["MODE_REQUEST_PATH"], overridden by env AETHER_MODE_REQUEST_PATH.
    """
    return str(os.getenv("AETHER_MODE_REQUEST_PATH", CONFIG.get("MODE_REQUEST_PATH", "mode_request.json")))


def read_mode_request() -> str | None:
    """Read a requested trade mode from the request file.

    Returns lowercased mode in {paper,pilot,live} or None.
    """
    path = _mode_request_path()
    try:
        if not os.path.exists(path):
            return None
        with open(path, "r", encoding="utf-8") as f:
            obj = json.load(f)
        mode = str(obj.get("requested_mode", "") or "").strip().lower()
        if mode in ("paper", "pilot", "live"):
            return mode
        return None
    except Exception:
        return None


def write_mode_request(mode: str) -> bool:
    """Write a requested trade mode to the request file."""
    m = str(mode or "").strip().lower()
    if m not in ("paper", "pilot", "live"):
        return False
    path = _mode_request_path()
    try:
        with open(path, "w", encoding="utf-8") as f:
            json.dump({"requested_mode": m, "ts": int(__import__("time").time())}, f)
        return True
    except Exception:
        return False

def load_env_keys() -> None:
    env_map = {
        "OPENAI_API_KEY": "OPENAI_API_KEY",
        "GROK_API_KEY": "GROK_API_KEY",
        "CLAUDE_API_KEY": "CLAUDE_API_KEY",
        "GEMINI_API_KEY": "GEMINI_API_KEY",
        "COINBASE_API_KEY": "COINBASE_API_KEY",
        "COINBASE_API_SECRET": "COINBASE_API_SECRET",
        "KRAKEN_API_KEY": "KRAKEN_API_KEY",
        "KRAKEN_API_SECRET": "KRAKEN_API_SECRET",
        "BINANCEUS_API_KEY": "BINANCEUS_API_KEY",
        "BINANCEUS_API_SECRET": "BINANCEUS_API_SECRET",
        "BTCC_API_KEY": "BTCC_API_KEY",
        "BTCC_API_SECRET": "BTCC_API_SECRET",
    }
    for k, env in env_map.items():
        v = os.getenv(env)
        if v:
            CONFIG[k] = v

    # Interactive prompt for pilot/live sessions (preferred).
    if str(CONFIG.get("TRADE_MODE", "paper")).lower() in ("pilot", "live") and bool(CONFIG.get("INTERACTIVE_KEY_PROMPT", True)):
        def _need(k: str) -> bool:
            return not str(CONFIG.get(k, "") or "").strip()

        # Only prompt for venues that are enabled.
        venue_keys = []
        if bool(CONFIG.get("COINBASE_ENABLED", True)):
            venue_keys += [("COINBASE_API_KEY", "Coinbase API Key"), ("COINBASE_API_SECRET", "Coinbase API Secret")]
        if bool(CONFIG.get("KRAKEN_ENABLED", True)):
            venue_keys += [("KRAKEN_API_KEY", "Kraken API Key"), ("KRAKEN_API_SECRET", "Kraken API Secret")]
        if bool(CONFIG.get("BINANCEUS_ENABLED", True)):
            venue_keys += [("BINANCEUS_API_KEY", "Binance.US API Key"), ("BINANCEUS_API_SECRET", "Binance.US API Secret")]
        venue_keys += [("BTCC_API_KEY", "BTCC API Key"), ("BTCC_API_SECRET", "BTCC API Secret")]

        for key, label in venue_keys:
            if _need(key):
                try:
                    CONFIG[key] = input(f"Enter {label}: ").strip()
                except Exception:
                    # Non-interactive environment: fail closed (keys stay empty).
                    pass

