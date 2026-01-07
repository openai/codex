from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal

ApprovalMode = Literal["never", "on-request", "on-failure", "untrusted"]
SandboxMode = Literal["read-only", "workspace-write", "danger-full-access"]
ModelReasoningEffort = Literal["minimal", "low", "medium", "high", "xhigh"]


@dataclass(slots=True)
class CodexOptions:
    codex_path_override: str | None = None
    base_url: str | None = None
    api_key: str | None = None
    # Environment variables passed to the Codex CLI process.
    env: dict[str, str] | None = None


@dataclass(slots=True)
class ThreadOptions:
    model: str | None = None
    sandbox_mode: SandboxMode | None = None
    working_directory: str | None = None
    skip_git_repo_check: bool | None = None
    model_reasoning_effort: ModelReasoningEffort | None = None
    network_access_enabled: bool | None = None
    web_search_enabled: bool | None = None
    approval_policy: ApprovalMode | None = None
    additional_directories: list[str] = field(default_factory=list)


@dataclass(slots=True)
class TurnOptions:
    output_schema: object | None = None
    signal: object | None = None
