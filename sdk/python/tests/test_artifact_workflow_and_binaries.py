from __future__ import annotations

import ast
import importlib.util
import json
import sys
import tomllib
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[1]


def _load_update_script_module():
    script_path = ROOT / "scripts" / "update_sdk_artifacts.py"
    spec = importlib.util.spec_from_file_location("update_sdk_artifacts", script_path)
    if spec is None or spec.loader is None:
        raise AssertionError(f"Failed to load script module: {script_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_generation_has_single_maintenance_entrypoint_script() -> None:
    scripts = sorted(p.name for p in (ROOT / "scripts").glob("*.py"))
    assert scripts == ["update_sdk_artifacts.py"]


def test_generate_types_wires_all_generation_steps() -> None:
    source = (ROOT / "scripts" / "update_sdk_artifacts.py").read_text()
    tree = ast.parse(source)

    generate_types_fn = next(
        (node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == "generate_types"),
        None,
    )
    assert generate_types_fn is not None

    calls: list[str] = []
    for node in generate_types_fn.body:
        if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call):
            fn = node.value.func
            if isinstance(fn, ast.Name):
                calls.append(fn.id)

    assert calls == [
        "generate_v2_all",
        "generate_notification_registry",
        "generate_public_api_flat_methods",
    ]


def test_schema_normalization_only_flattens_string_literal_oneofs() -> None:
    script = _load_update_script_module()
    schema = json.loads(
        (
            ROOT.parent.parent
            / "codex-rs"
            / "app-server-protocol"
            / "schema"
            / "json"
            / "codex_app_server_protocol.v2.schemas.json"
        ).read_text()
    )

    definitions = schema["definitions"]
    flattened = [
        name
        for name, definition in definitions.items()
        if isinstance(definition, dict)
        and script._flatten_string_enum_one_of(definition.copy())
    ]

    assert flattened == [
        "AuthMode",
        "CommandExecOutputStream",
        "ExperimentalFeatureStage",
        "InputModality",
        "MessagePhase",
    ]


def test_runtime_package_template_has_no_checked_in_binaries() -> None:
    runtime_root = ROOT.parent / "python-runtime" / "src" / "codex_cli_bin"
    assert sorted(
        path.name
        for path in runtime_root.rglob("*")
        if path.is_file() and "__pycache__" not in path.parts
    ) == ["__init__.py"]


def test_runtime_package_builds_platform_specific_wheels() -> None:
    pyproject = tomllib.loads((ROOT.parent / "python-runtime" / "pyproject.toml").read_text())
    hook_source = (ROOT.parent / "python-runtime" / "hatch_build.py").read_text()
    hook_tree = ast.parse(hook_source)
    initialize_fn = next(
        node
        for node in ast.walk(hook_tree)
        if isinstance(node, ast.FunctionDef) and node.name == "initialize"
    )
    build_data_assignments = {
        node.targets[0].slice.value: node.value.value
        for node in initialize_fn.body
        if isinstance(node, ast.Assign)
        and len(node.targets) == 1
        and isinstance(node.targets[0], ast.Subscript)
        and isinstance(node.targets[0].value, ast.Name)
        and node.targets[0].value.id == "build_data"
        and isinstance(node.targets[0].slice, ast.Constant)
        and isinstance(node.targets[0].slice.value, str)
        and isinstance(node.value, ast.Constant)
    }

    assert pyproject["tool"]["hatch"]["build"]["targets"]["wheel"] == {
        "packages": ["src/codex_cli_bin"],
        "include": ["src/codex_cli_bin/bin/**"],
        "hooks": {"custom": {}},
    }
    assert build_data_assignments == {"pure_python": False, "infer_tag": True}


def test_stage_runtime_release_copies_binary_and_sets_version(tmp_path: Path) -> None:
    script = _load_update_script_module()
    fake_binary = tmp_path / script.runtime_binary_name()
    fake_binary.write_text("fake codex\n")

    staged = script.stage_python_runtime_package(
        tmp_path / "runtime-stage",
        "1.2.3",
        fake_binary,
    )

    assert staged == tmp_path / "runtime-stage"
    assert script.staged_runtime_bin_path(staged).read_text() == "fake codex\n"
    assert 'version = "1.2.3"' in (staged / "pyproject.toml").read_text()


def test_stage_sdk_release_injects_exact_runtime_pin(tmp_path: Path) -> None:
    script = _load_update_script_module()
    staged = script.stage_python_sdk_package(tmp_path / "sdk-stage", "0.2.1", "1.2.3")

    pyproject = (staged / "pyproject.toml").read_text()
    assert 'version = "0.2.1"' in pyproject
    assert '"codex-cli-bin==1.2.3"' in pyproject
    assert not any((staged / "src" / "codex_app_server").glob("bin/**"))


def test_default_runtime_is_resolved_from_installed_runtime_package(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    from codex_app_server import client as client_module

    fake_binary = tmp_path / ("codex.exe" if client_module.os.name == "nt" else "codex")
    fake_binary.write_text("")
    monkeypatch.setattr(client_module, "_installed_codex_path", lambda: fake_binary)

    config = client_module.AppServerConfig()
    assert config.codex_bin is None
    assert client_module._resolve_codex_bin(config) == fake_binary
