from __future__ import annotations
import re, json, statistics
from typing import Dict, Any, List, Optional
from .config import CONFIG
from .utils import log

try:
    import aiohttp
except Exception:
    aiohttp = None  # type: ignore

def _first_float(text: str) -> Optional[float]:
    m = re.search(r"[-+]?\d+(?:\.\d+)?", text)
    if not m: return None
    try: return float(m.group(0))
    except Exception: return None

def enabled_judges() -> List[str]:
    out = []
    for j in CONFIG.get("AI_JURY", []):
        if j == "openai" and CONFIG.get("OPENAI_API_KEY"): out.append(j)
        elif j == "grok" and CONFIG.get("GROK_API_KEY"): out.append(j)
        elif j == "claude" and CONFIG.get("CLAUDE_API_KEY"): out.append(j)
        elif j == "gemini" and CONFIG.get("GEMINI_API_KEY"): out.append(j)
    return out

async def judge_number(prompt: str) -> Dict[str, Any]:
    if aiohttp is None:
        return {"votes": [], "mean": 0.0, "n": 0, "error": "aiohttp_missing"}
    jury = enabled_judges()
    if not jury:
        return {"votes": [], "mean": 0.0, "n": 0, "error": "no_judges_enabled"}

    votes: List[float] = []
    timeout = float(CONFIG.get("AI_TIMEOUT_SEC", 6))

    async with aiohttp.ClientSession() as s:
        for name in jury:
            try:
                if name in ("openai", "grok"):
                    url = "https://api.openai.com/v1/chat/completions" if name == "openai" else "https://api.x.ai/v1/chat/completions"
                    key = CONFIG.get("OPENAI_API_KEY") if name == "openai" else CONFIG.get("GROK_API_KEY")
                    model = "gpt-4o-mini" if name == "openai" else "grok-beta"
                    payload = {"model": model, "messages":[{"role":"user","content":prompt}], "temperature":0.0, "max_tokens": 16}
                    headers = {"Authorization": f"Bearer {key}", "Content-Type":"application/json"}
                    async with s.post(url, json=payload, headers=headers, timeout=timeout) as r:
                        txt = await r.text()
                        try:
                            j = json.loads(txt)
                            content = j["choices"][0]["message"]["content"]
                            v = _first_float(str(content))
                        except Exception:
                            v = _first_float(txt)
                        if v is not None: votes.append(float(v))
                else:
                    # Claude/Gemini minimal: extract float from raw response (you can harden this later)
                    if name == "claude":
                        url = "https://api.anthropic.com/v1/messages"
                        headers = {"x-api-key": CONFIG.get("CLAUDE_API_KEY"), "anthropic-version":"2023-06-01", "Content-Type":"application/json"}
                        payload = {"model":"claude-3.5-sonnet", "max_tokens": 16, "temperature":0.0, "messages":[{"role":"user","content":prompt}]}
                        async with s.post(url, json=payload, headers=headers, timeout=timeout) as r:
                            txt = await r.text()
                            v = _first_float(txt)
                            if v is not None: votes.append(float(v))
                    elif name == "gemini":
                        url = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent?key={CONFIG.get('GEMINI_API_KEY')}"
                        headers = {"Content-Type":"application/json"}
                        payload = {"contents":[{"parts":[{"text": prompt}]}], "generationConfig":{"temperature":0.0,"maxOutputTokens":16}}
                        async with s.post(url, json=payload, headers=headers, timeout=timeout) as r:
                            txt = await r.text()
                            v = _first_float(txt)
                            if v is not None: votes.append(float(v))
            except Exception as e:
                log.debug(f"[JURY] {name} failed: {e}")

    return {"votes": votes, "mean": float(statistics.mean(votes)) if votes else 0.0, "n": len(votes)}
