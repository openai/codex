from __future__ import annotations
from typing import Dict, Any, List
import time
try:
    import feedparser  # type: ignore
except Exception:
    feedparser = None  # type: ignore
from .config import CONFIG
from .db import memorize
from .jury import judge_number

_last = 0.0

async def fetch_rss_sentiment() -> Dict[str, Any]:
    global _last
    if (not bool(CONFIG.get("RSS_ENABLED", True))) or feedparser is None:
        return {"sent": 0.0, "items": []}
    now = time.time()
    if now - _last < float(CONFIG.get("RSS_MIN_SECONDS", 300)):
        return {"sent": 0.0, "items": [], "cached": True}
    _last = now

    items: List[Dict[str, Any]] = []
    score = 0.0
    for name, meta in (CONFIG.get("RSS_FEEDS", {}) or {}).items():
        url = meta.get("url"); w = float(meta.get("weight", 1.0))
        if not url: continue
        d = feedparser.parse(url)
        for e in (d.entries or [])[:5]:
            title = getattr(e, "title", "")
            link = getattr(e, "link", "")
            items.append({"src": name, "title": title, "url": link})
            t = (title or "").lower()
            local = 0.0
            if any(k in t for k in ("hack","lawsuit","ban","collapse","liquidation")): local -= 0.6
            if any(k in t for k in ("approve","etf","record","adoption","bull")): local += 0.6
            score += w*local

    sent = float(max(-1.0, min(1.0, score / max(1.0, len(items)))))

    # optional jury refinement
    if items:
        j = await judge_number(
            "Return sentiment -1..+1 number only for crypto headlines:\n" +
            "\n".join([f"- {it['title']}" for it in items[:8]])
        )
        if j.get("n", 0):
            sent = float(max(-1.0, min(1.0, j.get("mean", sent))))

    memorize("sentiment", {"ts": int(time.time()), "sent": sent, "items": items[:20]})
    return {"sent": sent, "items": items[:20]}
