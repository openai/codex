"""Optional websocket streams.

Some exchanges are supported by ccxt.pro (websocket). This module is designed
to be strictly optional: if ccxt.pro isn't installed, we fall back to polling.

The Prime OMS uses this to reduce lag in order state transitions when possible.
"""

from __future__ import annotations

import asyncio
from typing import Any, AsyncIterator, Optional

from .utils import log

try:
    import ccxt.pro as ccxtpro  # type: ignore
except Exception:  # pragma: no cover
    ccxtpro = None  # type: ignore


async def watch_orders(exchange_id: str, *, api_key: str, secret: str, password: str | None = None) -> Optional[AsyncIterator[dict]]:
    """Return an async iterator of order updates, or None if unsupported."""
    if ccxtpro is None:
        return None
    try:
        ex_class = getattr(ccxtpro, exchange_id)
    except Exception:
        return None
    ex = ex_class({"apiKey": api_key, "secret": secret, "password": password or ""})

    async def _iter():
        try:
            while True:
                orders = await ex.watch_orders()
                # ccxt.pro returns a list in some cases
                if isinstance(orders, list):
                    for o in orders:
                        yield o
                elif isinstance(orders, dict):
                    yield orders
        except asyncio.CancelledError:
            raise
        except Exception as e:
            log.warning(f"[ws] watch_orders stopped: {e}")
        finally:
            try:
                await ex.close()
            except Exception:
                pass

    return _iter()
