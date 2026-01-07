from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass
class AbortSignal:
    aborted: bool = False
    reason: Any = None

    def is_set(self) -> bool:
        return self.aborted


class AbortController:
    def __init__(self) -> None:
        self._signal = AbortSignal()

    @property
    def signal(self) -> AbortSignal:
        return self._signal

    def abort(self, reason: Any = None) -> None:
        self._signal.aborted = True
        self._signal.reason = reason
