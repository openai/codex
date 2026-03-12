from __future__ import annotations

import importlib.resources as resources
import inspect
from typing import Any

from codex_app_server import AppServerConfig
from codex_app_server.models import InitializeResponse
from codex_app_server.public_api import AsyncCodex, AsyncThread, Codex, Thread


def _keyword_only_names(fn: object) -> list[str]:
    signature = inspect.signature(fn)
    return [
        param.name
        for param in signature.parameters.values()
        if param.kind == inspect.Parameter.KEYWORD_ONLY
    ]


def _assert_no_any_annotations(fn: object) -> None:
    signature = inspect.signature(fn)
    for param in signature.parameters.values():
        if param.annotation is Any:
            raise AssertionError(f"{fn} has public parameter typed as Any: {param.name}")
    if signature.return_annotation is Any:
        raise AssertionError(f"{fn} has public return annotation typed as Any")


def test_root_exports_app_server_config() -> None:
    assert AppServerConfig.__name__ == "AppServerConfig"


def test_package_includes_py_typed_marker() -> None:
    marker = resources.files("codex_app_server").joinpath("py.typed")
    assert marker.is_file()


def test_generated_public_signatures_are_snake_case_and_typed() -> None:
    expected = {
        Codex.thread_start: [
            "approval_policy",
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
        ],
        Codex.thread_list: [
            "archived",
            "cursor",
            "cwd",
            "limit",
            "model_providers",
            "search_term",
            "sort_key",
            "source_kinds",
        ],
        Codex.thread_resume: [
            "approval_policy",
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
            "base_instructions",
            "config",
            "cwd",
            "developer_instructions",
            "ephemeral",
            "model",
            "model_provider",
            "sandbox",
            "service_tier",
        ],
        Thread.turn: [
            "approval_policy",
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
        ],
        AsyncCodex.thread_list: [
            "archived",
            "cursor",
            "cwd",
            "limit",
            "model_providers",
            "search_term",
            "sort_key",
            "source_kinds",
        ],
        AsyncCodex.thread_resume: [
            "approval_policy",
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
            "base_instructions",
            "config",
            "cwd",
            "developer_instructions",
            "ephemeral",
            "model",
            "model_provider",
            "sandbox",
            "service_tier",
        ],
        AsyncThread.turn: [
            "approval_policy",
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
        assert all(name == name.lower() for name in actual), f"non snake_case kwargs in {fn}: {actual}"
        _assert_no_any_annotations(fn)


def test_lifecycle_methods_are_codex_scoped() -> None:
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
    parsed = Codex._parse_initialize(InitializeResponse.model_validate({"userAgent": "codex-cli/1.2.3"}))
    assert parsed.user_agent == "codex-cli/1.2.3"
    assert parsed.server_name == "codex-cli"
    assert parsed.server_version == "1.2.3"


def test_initialize_metadata_requires_non_empty_information() -> None:
    try:
        Codex._parse_initialize(InitializeResponse.model_validate({}))
    except RuntimeError as exc:
        assert "missing required metadata" in str(exc)
    else:
        raise AssertionError("expected RuntimeError when initialize metadata is missing")
