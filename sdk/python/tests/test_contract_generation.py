from __future__ import annotations

import ast
import os
import runpy
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
GENERATED_TARGETS = [
    Path("src/openai_codex/generated/notification_registry.py"),
    Path("src/openai_codex/generated/v2_all.py"),
    Path("src/openai_codex/api.py"),
]


def _snapshot_target(root: Path, rel_path: Path) -> dict[str, bytes] | bytes | None:
    """Capture one generated artifact so regeneration drift is easy to compare."""
    target = root / rel_path
    if not target.exists():
        return None
    if target.is_file():
        return target.read_bytes()

    snapshot: dict[str, bytes] = {}
    for path in sorted(target.rglob("*")):
        if path.is_file() and "__pycache__" not in path.parts:
            snapshot[str(path.relative_to(target))] = path.read_bytes()
    return snapshot


def _snapshot_targets(root: Path) -> dict[str, dict[str, bytes] | bytes | None]:
    """Capture all checked-in generated artifacts before and after regeneration."""
    return {str(rel_path): _snapshot_target(root, rel_path) for rel_path in GENERATED_TARGETS}


@pytest.mark.parametrize("format_output", [True, False])
def test_generated_files_are_up_to_date(tmp_path, format_output):
    """Both generation modes preserve the reviewed API, with optional formatting."""
    before = _snapshot_targets(ROOT)
    sdk = tmp_path / "sdk" / "python"
    sdk.mkdir(parents=True)
    for directory in ("src", "scripts"):
        shutil.copytree(
            ROOT / directory, sdk / directory, ignore=shutil.ignore_patterns("__pycache__")
        )
    for name in ("pyproject.toml", "release_version.py"):
        shutil.copyfile(ROOT / name, sdk / name)

    env = os.environ.copy()
    python_bin = str(Path(sys.executable).parent)
    env["PATH"] = f"{python_bin}{os.pathsep}{env.get('PATH', '')}"

    command = [
        sys.executable,
        "scripts/update_sdk_artifacts.py",
        "generate-types",
        "--schema-dir",
        str(ROOT.parents[1] / "codex-rs/app-server-protocol/schema/json"),
    ]
    if not format_output:
        command.append("--no-format")
    subprocess.run(
        command,
        cwd=sdk,
        check=True,
        env=env,
    )

    after = _snapshot_targets(sdk)
    if format_output:
        assert before == after, "Generated files drifted after regeneration"
    else:
        assert [ast.dump(ast.parse((ROOT / path).read_bytes())) for path in GENERATED_TARGETS] == [
            ast.dump(ast.parse((sdk / path).read_bytes())) for path in GENERATED_TARGETS
        ]


@pytest.mark.parametrize("mode", ["repository", "scratch", "experimental"])
def test_schema_refresh_only_updates_python_for_repository_schemas(monkeypatch, tmp_path, mode):
    script = ROOT.parents[1] / "codex-rs/app-server-protocol/scripts/write_schema_fixtures.py"
    arguments = {
        "repository": [],
        "scratch": ["--schema-root", str(tmp_path / "schema")],
        "experimental": ["--experimental"],
    }[mode]
    calls = []
    monkeypatch.setattr(sys, "argv", [str(script), *arguments])
    monkeypatch.setattr(subprocess, "run", lambda args, **kwargs: calls.append((args, kwargs)))

    runpy.run_path(str(script), run_name="__main__")

    assert [args[0] for args, _kwargs in calls] == (
        ["cargo", "uv"] if mode == "repository" else ["cargo"]
    )
    assert all(kwargs["check"] for _args, kwargs in calls)
    if mode == "repository":
        assert calls[1][0][-3:] == [
            "generate-types",
            "--schema-dir",
            str(ROOT.parents[1] / "codex-rs/app-server-protocol/schema/json"),
        ]


def test_schema_generation_failure_does_not_update_python(monkeypatch):
    script = ROOT.parents[1] / "codex-rs/app-server-protocol/scripts/write_schema_fixtures.py"
    calls = []

    def fail(args, **_kwargs):
        calls.append(args[0])
        raise subprocess.CalledProcessError(1, args)

    monkeypatch.setattr(sys, "argv", [str(script)])
    monkeypatch.setattr(subprocess, "run", fail)
    with pytest.raises(subprocess.CalledProcessError):
        runpy.run_path(str(script), run_name="__main__")
    assert calls == ["cargo"]


@pytest.mark.parametrize(
    "payload",
    [
        {"type": "modelProviderNotConfigured", "modelProvider": "old-provider"},
        {
            "type": "modelProviderMismatch",
            "modelProvider": "old-provider",
            "currentModelProvider": "current-provider",
        },
        {
            "type": "modelProviderPolicyConflict",
            "modelProvider": "old-provider",
            "requiredModelProvider": "required-provider",
        },
    ],
)
def test_model_provider_errors_round_trip_wire_names(payload):
    from openai_codex.generated.v2_all import ModelProviderError

    error = ModelProviderError.model_validate(payload)
    assert error.model_dump(by_alias=True) == payload
