from __future__ import annotations

from enum import Enum
from typing import NoReturn

from .generated.v2_all import (
    DangerFullAccessSandboxPolicy,
    ReadOnlySandboxPolicy,
    SandboxMode,
    SandboxPolicy,
    WorkspaceWriteSandboxPolicy,
)


class Sandbox(str, Enum):
    """Preset filesystem access levels for threads and turns.

    `read_only` allows file reads without writes. `workspace_write` is the
    normal default for projects with a recorded trust decision and allows
    writes inside the workspace and configured writable roots. `full_access`
    removes filesystem access restrictions.
    """

    read_only = "read-only"
    workspace_write = "workspace-write"
    full_access = "full-access"


def _require_sandbox(sandbox: Sandbox) -> None:
    if isinstance(sandbox, Sandbox):
        return
    options = ", ".join(f"Sandbox.{value.name}" for value in Sandbox)
    raise ValueError(f"sandbox must be one of: {options}")


def _sandbox_mode(sandbox: Sandbox | None) -> SandboxMode | None:
    """Translate a public preset to the thread lifecycle wire mode."""
    if sandbox is None:
        return None
    _require_sandbox(sandbox)

    match sandbox:
        case Sandbox.read_only:
            return SandboxMode.read_only
        case Sandbox.workspace_write:
            return SandboxMode.workspace_write
        case Sandbox.full_access:
            return SandboxMode.danger_full_access
        case _:
            return _assert_never_sandbox(sandbox)


def _sandbox_policy(sandbox: Sandbox | None) -> SandboxPolicy | None:
    """Translate a public preset to the turn override wire policy."""
    if sandbox is None:
        return None
    _require_sandbox(sandbox)

    match sandbox:
        case Sandbox.read_only:
            return SandboxPolicy(
                root=ReadOnlySandboxPolicy(type="readOnly"),
            )
        case Sandbox.workspace_write:
            return SandboxPolicy(
                root=WorkspaceWriteSandboxPolicy(type="workspaceWrite"),
            )
        case Sandbox.full_access:
            return SandboxPolicy(
                root=DangerFullAccessSandboxPolicy(type="dangerFullAccess"),
            )
        case _:
            return _assert_never_sandbox(sandbox)


def _assert_never_sandbox(sandbox: NoReturn) -> NoReturn:
    """Make sandbox mapping exhaustive for static type checkers."""
    raise AssertionError(f"Unhandled sandbox: {sandbox!r}")
