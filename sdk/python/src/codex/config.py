from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum
from typing import Mapping, Optional, TYPE_CHECKING

if TYPE_CHECKING:  # pragma: no cover - typing only
    from pydantic import BaseModel as PydanticBaseModel
    SchemaInput = Mapping[str, object] | type[PydanticBaseModel] | PydanticBaseModel
else:
    SchemaInput = Mapping[str, object]


class ApprovalMode(StrEnum):
    NEVER = "never"
    ON_REQUEST = "on-request"
    ON_FAILURE = "on-failure"
    UNTRUSTED = "untrusted"


class SandboxMode(StrEnum):
    READ_ONLY = "read-only"
    WORKSPACE_WRITE = "workspace-write"
    DANGER_FULL_ACCESS = "danger-full-access"


@dataclass(frozen=True, slots=True)
class CodexOptions:
    codex_path_override: Optional[str] = None
    base_url: Optional[str] = None
    api_key: Optional[str] = None


@dataclass(frozen=True, slots=True)
class ThreadOptions:
    model: Optional[str] = None
    sandbox_mode: Optional[SandboxMode] = None
    working_directory: Optional[str] = None
    skip_git_repo_check: bool = False


@dataclass(frozen=True, slots=True)
class TurnOptions:
    output_schema: Optional[SchemaInput] = None
