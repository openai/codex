from __future__ import annotations

import importlib.resources as resources
import inspect
import tomllib
from pathlib import Path
from typing import Any

import codex_app_server
import codex_app_server.types as public_types
from codex_app_server import (
    AppServerConfig,
    AsyncCodex,
    AsyncThread,
    Codex,
    RunResult,
    Thread,
)
from codex_app_server.types import InitializeResponse

EXPECTED_ROOT_EXPORTS = [
    "__version__",
    "AppServerConfig",
    "Codex",
    "AsyncCodex",
    "Thread",
    "AsyncThread",
    "TurnHandle",
    "AsyncTurnHandle",
    "RunResult",
    "Input",
    "InputItem",
    "TextInput",
    "ImageInput",
    "LocalImageInput",
    "SkillInput",
    "MentionInput",
    "retry_on_overload",
    "AppServerError",
    "TransportClosedError",
    "JsonRpcError",
    "AppServerRpcError",
    "ParseError",
    "InvalidRequestError",
    "MethodNotFoundError",
    "InvalidParamsError",
    "InternalRpcError",
    "ServerBusyError",
    "RetryLimitExceededError",
    "is_retryable_error",
]

EXPECTED_TYPES_EXPORTS = [
    "ApprovalsReviewer",
    "AskForApproval",
    "InitializeResponse",
    "JsonObject",
    "ModelListResponse",
    "Notification",
    "Personality",
    "PlanType",
    "ReasoningEffort",
    "ReasoningSummary",
    "SandboxMode",
    "SandboxPolicy",
    "SortDirection",
    "ThreadArchiveResponse",
    "ThreadCompactStartResponse",
    "ThreadItem",
    "ThreadListCwdFilter",
    "ThreadListResponse",
    "ThreadReadResponse",
    "ThreadSetNameResponse",
    "ThreadSortKey",
    "ThreadSource",
    "ThreadSourceKind",
    "ThreadStartSource",
    "ThreadTokenUsage",
    "ThreadTokenUsageUpdatedNotification",
    "Turn",
    "TurnCompletedNotification",
    "TurnInterruptResponse",
    "TurnStatus",
    "TurnSteerResponse",
]


def _keyword_only_names(fn: object) -> list[str]:
    """Return only user-facing keyword-only parameter names for a public method."""
    signature = inspect.signature(fn)
    return [
        param.name
        for param in signature.parameters.values()
        if param.kind == inspect.Parameter.KEYWORD_ONLY
    ]


def _assert_no_any_annotations(fn: object) -> None:
    """Reject loose annotations on public wrapper methods."""
    signature = inspect.signature(fn)
    for param in signature.parameters.values():
        if param.annotation is Any:
            raise AssertionError(
                f"{fn} has public parameter typed as Any: {param.name}"
            )
    if signature.return_annotation is Any:
        raise AssertionError(f"{fn} has public return annotation typed as Any")


def test_root_exports_app_server_config() -> None:
    """The root package should expose the process configuration object."""
    assert AppServerConfig.__name__ == "AppServerConfig"


def test_root_exports_run_result() -> None:
    """The root package should expose the common-case run result wrapper."""
    assert RunResult.__name__ == "RunResult"


def test_package_and_default_client_versions_follow_project_version() -> None:
    """The importable package version should stay aligned with pyproject metadata."""
    pyproject_path = Path(__file__).resolve().parents[1] / "pyproject.toml"
    pyproject = tomllib.loads(pyproject_path.read_text())

    assert codex_app_server.__version__ == pyproject["project"]["version"]
    assert AppServerConfig().client_version == codex_app_server.__version__


def test_package_includes_py_typed_marker() -> None:
    """The wheel should advertise that inline type information is available."""
    marker = resources.files("codex_app_server").joinpath("py.typed")
    assert marker.is_file()


def test_package_root_exports_only_public_api() -> None:
    """The package root should expose the supported SDK surface, not internals."""
    assert codex_app_server.__all__ == EXPECTED_ROOT_EXPORTS
    assert {
        name: hasattr(codex_app_server, name) for name in EXPECTED_ROOT_EXPORTS
    } == {name: True for name in EXPECTED_ROOT_EXPORTS}
    assert {
        "AppServerClient": hasattr(codex_app_server, "AppServerClient"),
        "AsyncAppServerClient": hasattr(codex_app_server, "AsyncAppServerClient"),
        "InitializeResponse": hasattr(codex_app_server, "InitializeResponse"),
        "ThreadStartParams": hasattr(codex_app_server, "ThreadStartParams"),
        "TurnStartParams": hasattr(codex_app_server, "TurnStartParams"),
        "TurnCompletedNotification": hasattr(
            codex_app_server, "TurnCompletedNotification"
        ),
        "TurnStatus": hasattr(codex_app_server, "TurnStatus"),
    } == {
        "AppServerClient": False,
        "AsyncAppServerClient": False,
        "InitializeResponse": False,
        "ThreadStartParams": False,
        "TurnStartParams": False,
        "TurnCompletedNotification": False,
        "TurnStatus": False,
    }


def test_package_star_import_matches_public_api() -> None:
    """Star imports should follow the same explicit public API list."""
    namespace: dict[str, object] = {}
    exec("from codex_app_server import *", namespace)

    exported = set(namespace) - {"__builtins__"}
    assert exported == set(EXPECTED_ROOT_EXPORTS)


def test_types_module_exports_curated_public_types() -> None:
    """The public type module should be the supported place for app-server models."""
    assert public_types.__all__ == EXPECTED_TYPES_EXPORTS
    assert {name: hasattr(public_types, name) for name in EXPECTED_TYPES_EXPORTS} == {
        name: True for name in EXPECTED_TYPES_EXPORTS
    }


def test_types_star_import_matches_public_types() -> None:
    """Star imports from the type module should match its explicit export list."""
    namespace: dict[str, object] = {}
    exec("from codex_app_server.types import *", namespace)

    exported = set(namespace) - {"__builtins__"}
    assert exported == set(EXPECTED_TYPES_EXPORTS)


def test_examples_use_public_import_surfaces() -> None:
    """Examples should teach users the public root and type-module imports only."""
    examples_root = Path(__file__).resolve().parents[1] / "examples"
    private_import_markers = [
        "codex_app_server.api",
        "codex_app_server.client",
        "codex_app_server.generated",
        "codex_app_server.models",
        "codex_app_server.retry",
    ]

    offenders = {
        str(path.relative_to(examples_root)): marker
        for path in examples_root.rglob("*.py")
        for marker in private_import_markers
        if marker in path.read_text()
    }

    assert offenders == {}


def test_generated_public_signatures_are_snake_case_and_typed() -> None:
    """Generated convenience methods should expose typed Pythonic keyword names."""
    expected = {
        Codex.thread_start: [
            "approval_policy",
            "approvals_reviewer",
            "base_instructions",
            "config",
            "cwd",
            "developer_instructions",
            "ephemeral",
            "model",
            "model_provider",
            "personality",
            "sandbox",
            "service_name",
            "service_tier",
            "session_start_source",
            "thread_source",
        ],
        Codex.thread_list: [
            "archived",
            "cursor",
            "cwd",
            "limit",
            "model_providers",
            "search_term",
            "sort_direction",
            "sort_key",
            "source_kinds",
            "use_state_db_only",
        ],
        Codex.thread_resume: [
            "approval_policy",
            "approvals_reviewer",
            "base_instructions",
            "config",
            "cwd",
            "developer_instructions",
            "model",
            "model_provider",
            "personality",
            "sandbox",
            "service_tier",
        ],
        Codex.thread_fork: [
            "approval_policy",
            "approvals_reviewer",
            "base_instructions",
            "config",
            "cwd",
            "developer_instructions",
            "ephemeral",
            "model",
            "model_provider",
            "sandbox",
            "service_tier",
            "thread_source",
        ],
        Thread.turn: [
            "approval_policy",
            "approvals_reviewer",
            "cwd",
            "effort",
            "model",
            "output_schema",
            "personality",
            "sandbox_policy",
            "service_tier",
            "summary",
        ],
        Thread.run: [
            "approval_policy",
            "approvals_reviewer",
            "cwd",
            "effort",
            "model",
            "output_schema",
            "personality",
            "sandbox_policy",
            "service_tier",
            "summary",
        ],
        AsyncCodex.thread_start: [
            "approval_policy",
            "approvals_reviewer",
            "base_instructions",
            "config",
            "cwd",
            "developer_instructions",
            "ephemeral",
            "model",
            "model_provider",
            "personality",
            "sandbox",
            "service_name",
            "service_tier",
            "session_start_source",
            "thread_source",
        ],
        AsyncCodex.thread_list: [
            "archived",
            "cursor",
            "cwd",
            "limit",
            "model_providers",
            "search_term",
            "sort_direction",
            "sort_key",
            "source_kinds",
            "use_state_db_only",
        ],
        AsyncCodex.thread_resume: [
            "approval_policy",
            "approvals_reviewer",
            "base_instructions",
            "config",
            "cwd",
            "developer_instructions",
            "model",
            "model_provider",
            "personality",
            "sandbox",
            "service_tier",
        ],
        AsyncCodex.thread_fork: [
            "approval_policy",
            "approvals_reviewer",
            "base_instructions",
            "config",
            "cwd",
            "developer_instructions",
            "ephemeral",
            "model",
            "model_provider",
            "sandbox",
            "service_tier",
            "thread_source",
        ],
        AsyncThread.turn: [
            "approval_policy",
            "approvals_reviewer",
            "cwd",
            "effort",
            "model",
            "output_schema",
            "personality",
            "sandbox_policy",
            "service_tier",
            "summary",
        ],
        AsyncThread.run: [
            "approval_policy",
            "approvals_reviewer",
            "cwd",
            "effort",
            "model",
            "output_schema",
            "personality",
            "sandbox_policy",
            "service_tier",
            "summary",
        ],
    }

    for fn, expected_kwargs in expected.items():
        actual = _keyword_only_names(fn)
        assert actual == expected_kwargs, f"unexpected kwargs for {fn}: {actual}"
        assert all(name == name.lower() for name in actual), (
            f"non snake_case kwargs in {fn}: {actual}"
        )
        _assert_no_any_annotations(fn)


def test_lifecycle_methods_are_codex_scoped() -> None:
    """Lifecycle operations should hang off the client rather than thread objects."""
    assert hasattr(Codex, "thread_resume")
    assert hasattr(Codex, "thread_fork")
    assert hasattr(Codex, "thread_archive")
    assert hasattr(Codex, "thread_unarchive")
    assert hasattr(AsyncCodex, "thread_resume")
    assert hasattr(AsyncCodex, "thread_fork")
    assert hasattr(AsyncCodex, "thread_archive")
    assert hasattr(AsyncCodex, "thread_unarchive")
    assert not hasattr(Codex, "thread")
    assert not hasattr(AsyncCodex, "thread")

    assert not hasattr(Thread, "resume")
    assert not hasattr(Thread, "fork")
    assert not hasattr(Thread, "archive")
    assert not hasattr(Thread, "unarchive")
    assert not hasattr(AsyncThread, "resume")
    assert not hasattr(AsyncThread, "fork")
    assert not hasattr(AsyncThread, "archive")
    assert not hasattr(AsyncThread, "unarchive")

    for fn in (
        Codex.thread_archive,
        Codex.thread_unarchive,
        AsyncCodex.thread_archive,
        AsyncCodex.thread_unarchive,
    ):
        _assert_no_any_annotations(fn)


def test_initialize_metadata_parses_user_agent_shape() -> None:
    """Initialize metadata should accept the legacy user-agent-only payload shape."""
    payload = InitializeResponse.model_validate({"userAgent": "codex-cli/1.2.3"})
    parsed = Codex._validate_initialize(payload)
    assert parsed is payload
    assert parsed.userAgent == "codex-cli/1.2.3"
    assert parsed.serverInfo is not None
    assert parsed.serverInfo.name == "codex-cli"
    assert parsed.serverInfo.version == "1.2.3"


def test_initialize_metadata_requires_non_empty_information() -> None:
    """Initialize metadata should fail when the runtime gives no identity signal."""
    try:
        Codex._validate_initialize(InitializeResponse.model_validate({}))
    except RuntimeError as exc:
        assert "missing required metadata" in str(exc)
    else:
        raise AssertionError(
            "expected RuntimeError when initialize metadata is missing"
        )
