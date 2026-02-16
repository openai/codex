from __future__ import annotations

import random
import time
from typing import Callable, TypeVar

from .errors import is_retryable_error

T = TypeVar("T")


def retry_on_overload(
    op: Callable[[], T],
    *,
    max_attempts: int = 3,
    initial_delay_s: float = 0.25,
    max_delay_s: float = 2.0,
    jitter_ratio: float = 0.2,
) -> T:
    """Retry helper for transient server-overload errors.

    Raises the last exception when attempts are exhausted.
    """

    if max_attempts < 1:
        raise ValueError("max_attempts must be >= 1")

    delay = initial_delay_s
    attempt = 0
    while True:
        attempt += 1
        try:
            return op()
        except Exception as exc:
            if attempt >= max_attempts:
                raise
            if not is_retryable_error(exc):
                raise

            jitter = delay * jitter_ratio
            sleep_for = min(max_delay_s, delay) + random.uniform(-jitter, jitter)
            if sleep_for > 0:
                time.sleep(sleep_for)
            delay = min(max_delay_s, delay * 2)
