from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, Optional, Protocol


class CancellationEvent(Protocol):
    def is_set(self) -> bool:
        ...


@dataclass
class CodexOptions:
    codex_path_override: Optional[str] = None
    base_url: Optional[str] = None
    api_key: Optional[str] = None
    env: Optional[Dict[str, str]] = None


@dataclass
class ThreadOptions:
    model: Optional[str] = None
    sandbox_mode: Optional[str] = None
    working_directory: Optional[str] = None
    skip_git_repo_check: bool = False
    model_reasoning_effort: Optional[str] = None
    network_access_enabled: Optional[bool] = None
    web_search_enabled: Optional[bool] = None
    approval_policy: Optional[str] = None
    additional_directories: Optional[list[str]] = None


@dataclass
class TurnOptions:
    output_schema: Optional[object] = None
    cancellation_event: Optional[CancellationEvent] = None
