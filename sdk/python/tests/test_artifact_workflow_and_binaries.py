from __future__ import annotations

import ast
import importlib.util
import json
import platform
import sys
from pathlib import Path


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


def test_bundled_binaries_exist_for_all_supported_platforms() -> None:
    script = _load_update_script_module()
    for platform_key in script.PLATFORMS:
        bin_path = script.bundled_platform_bin_path(platform_key)
        assert bin_path.is_file(), f"Missing bundled binary: {bin_path}"


def test_default_runtime_uses_current_platform_bundled_binary() -> None:
    client_source = (ROOT / "src" / "codex_app_server" / "client.py").read_text()
    client_tree = ast.parse(client_source)

    # Keep this assertion source-level so it works in both PR2 (types foundation)
    # and PR3 (full SDK), regardless of runtime module wiring.
    app_server_config = next(
        (
            node
            for node in client_tree.body
            if isinstance(node, ast.ClassDef) and node.name == "AppServerConfig"
        ),
        None,
    )
    assert app_server_config is not None

    codex_bin_field = next(
        (
            node
            for node in app_server_config.body
            if isinstance(node, ast.AnnAssign)
            and isinstance(node.target, ast.Name)
            and node.target.id == "codex_bin"
        ),
        None,
    )
    assert codex_bin_field is not None
    assert isinstance(codex_bin_field.value, ast.Call)
    assert isinstance(codex_bin_field.value.func, ast.Name)
    assert codex_bin_field.value.func.id == "str"
    assert len(codex_bin_field.value.args) == 1
    bundled_call = codex_bin_field.value.args[0]
    assert isinstance(bundled_call, ast.Call)
    assert isinstance(bundled_call.func, ast.Name)
    assert bundled_call.func.id == "_bundled_codex_path"

    bin_root = (ROOT / "src" / "codex_app_server" / "bin").resolve()

    sys_name = platform.system().lower()
    machine = platform.machine().lower()
    is_arm = machine in {"arm64", "aarch64"}

    if sys_name.startswith("darwin"):
        platform_dir = "darwin-arm64" if is_arm else "darwin-x64"
        exe = "codex"
    elif sys_name.startswith("linux"):
        platform_dir = "linux-arm64" if is_arm else "linux-x64"
        exe = "codex"
    elif sys_name.startswith("windows"):
        platform_dir = "windows-arm64" if is_arm else "windows-x64"
        exe = "codex.exe"
    else:
        raise AssertionError(f"Unsupported platform in test: {sys_name}/{machine}")

    expected = (bin_root / platform_dir / exe).resolve()
    assert expected.is_file()
