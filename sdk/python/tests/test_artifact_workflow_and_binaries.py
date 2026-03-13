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
        (
            node
            for node in tree.body
            if isinstance(node, ast.FunctionDef) and node.name == "generate_types"
        ),
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


def test_python_codegen_schema_annotation_adds_stable_variant_titles() -> None:
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

    script._annotate_schema(schema)
    definitions = schema["definitions"]

    server_notification_titles = {
        variant.get("title")
        for variant in definitions["ServerNotification"]["oneOf"]
        if isinstance(variant, dict)
    }
    assert "ErrorServerNotification" in server_notification_titles
    assert "ThreadStartedServerNotification" in server_notification_titles
    assert "ErrorNotification" not in server_notification_titles
    assert "Thread/startedNotification" not in server_notification_titles

    ask_for_approval_titles = [
        variant.get("title") for variant in definitions["AskForApproval"]["oneOf"]
    ]
    assert ask_for_approval_titles == [
        "AskForApprovalValue",
        "GranularAskForApproval",
    ]

    reasoning_summary_titles = [
        variant.get("title") for variant in definitions["ReasoningSummary"]["oneOf"]
    ]
    assert reasoning_summary_titles == [
        "ReasoningSummaryValue",
        "NoneReasoningSummary",
    ]


def test_generate_v2_all_uses_titles_for_generated_names() -> None:
    source = (ROOT / "scripts" / "update_sdk_artifacts.py").read_text()
    assert "--use-title-as-name" in source
    assert "--use-annotated" in source
    assert "--formatters" in source
    assert "ruff-format" in source


def test_runtime_package_template_has_no_checked_in_binaries() -> None:
    runtime_root = ROOT.parent / "python-runtime" / "src" / "codex_cli_bin"
    assert sorted(
        path.name
        for path in runtime_root.rglob("*")
        if path.is_file() and "__pycache__" not in path.parts
    ) == ["__init__.py"]


def test_runtime_package_is_wheel_only_and_builds_platform_specific_wheels() -> None:
    pyproject = tomllib.loads(
        (ROOT.parent / "python-runtime" / "pyproject.toml").read_text()
    )
    hook_source = (ROOT.parent / "python-runtime" / "hatch_build.py").read_text()
    hook_tree = ast.parse(hook_source)
    initialize_fn = next(
        node
        for node in ast.walk(hook_tree)
        if isinstance(node, ast.FunctionDef) and node.name == "initialize"
    )

    sdist_guard = next(
        (
            node
            for node in initialize_fn.body
            if isinstance(node, ast.If)
            and isinstance(node.test, ast.Compare)
            and isinstance(node.test.left, ast.Attribute)
            and isinstance(node.test.left.value, ast.Name)
            and node.test.left.value.id == "self"
            and node.test.left.attr == "target_name"
            and len(node.test.ops) == 1
            and isinstance(node.test.ops[0], ast.Eq)
            and len(node.test.comparators) == 1
            and isinstance(node.test.comparators[0], ast.Constant)
            and node.test.comparators[0].value == "sdist"
        ),
        None,
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
    assert pyproject["tool"]["hatch"]["build"]["targets"]["sdist"] == {
        "hooks": {"custom": {}},
    }
    assert sdist_guard is not None
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


def test_stage_runtime_release_replaces_existing_staging_dir(tmp_path: Path) -> None:
    script = _load_update_script_module()
    staging_dir = tmp_path / "runtime-stage"
    old_file = staging_dir / "stale.txt"
    old_file.parent.mkdir(parents=True)
    old_file.write_text("stale")

    fake_binary = tmp_path / script.runtime_binary_name()
    fake_binary.write_text("fake codex\n")

    staged = script.stage_python_runtime_package(
        staging_dir,
        "1.2.3",
        fake_binary,
    )

    assert staged == staging_dir
    assert not old_file.exists()
    assert script.staged_runtime_bin_path(staged).read_text() == "fake codex\n"


def test_stage_core_sdk_release_sets_version_without_runtime_pin(tmp_path: Path) -> None:
    script = _load_update_script_module()
    staged = script.stage_python_core_sdk_package(tmp_path / "sdk-core-stage", "0.2.1")

    pyproject = (staged / "pyproject.toml").read_text()
    assert 'version = "0.2.1"' in pyproject
    assert 'name = "codex-app-server-sdk-core"' in pyproject
    assert "codex-cli-bin" not in pyproject
    assert not any((staged / "src" / "codex_app_server").glob("bin/**"))


def test_stage_bundled_sdk_release_injects_exact_core_and_runtime_pins(
    tmp_path: Path,
) -> None:
    script = _load_update_script_module()
    staged = script.stage_python_sdk_package(tmp_path / "sdk-stage", "0.2.1", "1.2.3")

    pyproject = (staged / "pyproject.toml").read_text()
    assert 'name = "codex-app-server-sdk"' in pyproject
    assert 'version = "0.2.1"' in pyproject
    assert '"codex-app-server-sdk-core==0.2.1"' in pyproject
    assert '"codex-cli-bin==1.2.3"' in pyproject


def test_stage_bundled_sdk_release_replaces_existing_staging_dir(tmp_path: Path) -> None:
    script = _load_update_script_module()
    staging_dir = tmp_path / "sdk-stage"
    old_file = staging_dir / "stale.txt"
    old_file.parent.mkdir(parents=True)
    old_file.write_text("stale")

    staged = script.stage_python_sdk_package(staging_dir, "0.2.1", "1.2.3")

    assert staged == staging_dir
    assert not old_file.exists()


def test_stage_sdk_core_runs_type_generation_before_staging(tmp_path: Path) -> None:
    script = _load_update_script_module()
    calls: list[str] = []
    args = script.parse_args(
        [
            "stage-sdk-core",
            str(tmp_path / "sdk-core-stage"),
        ]
    )

    def fake_generate_types() -> None:
        calls.append("generate_types")

    def fake_stage_core_sdk_package(_staging_dir: Path, _sdk_version: str) -> Path:
        calls.append("stage_sdk_core")
        return tmp_path / "sdk-core-stage"

    def fake_stage_sdk_package(
        _staging_dir: Path, _sdk_version: str, _runtime_version: str
    ) -> Path:
        raise AssertionError("bundled sdk staging should not run for stage-sdk-core")

    def fake_stage_runtime_package(
        _staging_dir: Path, _runtime_version: str, _runtime_binary: Path
    ) -> Path:
        raise AssertionError("runtime staging should not run for stage-sdk-core")

    def fake_current_sdk_version() -> str:
        return "0.2.0"

    ops = script.CliOps(
        generate_types=fake_generate_types,
        stage_python_core_sdk_package=fake_stage_core_sdk_package,
        stage_python_sdk_package=fake_stage_sdk_package,
        stage_python_runtime_package=fake_stage_runtime_package,
        current_sdk_version=fake_current_sdk_version,
    )

    script.run_command(args, ops)

    assert calls == ["generate_types", "stage_sdk_core"]


def test_stage_sdk_core_can_skip_type_generation(tmp_path: Path) -> None:
    script = _load_update_script_module()
    calls: list[str] = []
    args = script.parse_args(
        [
            "stage-sdk-core",
            str(tmp_path / "sdk-core-stage"),
            "--skip-generate-types",
        ]
    )

    def fake_generate_types() -> None:
        calls.append("generate_types")

    def fake_stage_core_sdk_package(_staging_dir: Path, _sdk_version: str) -> Path:
        calls.append("stage_sdk_core")
        return tmp_path / "sdk-core-stage"

    def fake_stage_sdk_package(
        _staging_dir: Path, _sdk_version: str, _runtime_version: str
    ) -> Path:
        raise AssertionError("bundled sdk staging should not run for stage-sdk-core")

    def fake_stage_runtime_package(
        _staging_dir: Path, _runtime_version: str, _runtime_binary: Path
    ) -> Path:
        raise AssertionError("runtime staging should not run for stage-sdk-core")

    def fake_current_sdk_version() -> str:
        return "0.2.0"

    ops = script.CliOps(
        generate_types=fake_generate_types,
        stage_python_core_sdk_package=fake_stage_core_sdk_package,
        stage_python_sdk_package=fake_stage_sdk_package,
        stage_python_runtime_package=fake_stage_runtime_package,
        current_sdk_version=fake_current_sdk_version,
    )

    script.run_command(args, ops)

    assert calls == ["stage_sdk_core"]


def test_stage_sdk_runs_type_generation_before_staging(tmp_path: Path) -> None:
    script = _load_update_script_module()
    calls: list[str] = []
    args = script.parse_args(
        [
            "stage-sdk",
            str(tmp_path / "sdk-stage"),
            "--runtime-version",
            "1.2.3",
        ]
    )

    def fake_generate_types() -> None:
        calls.append("generate_types")

    def fake_stage_core_sdk_package(_staging_dir: Path, _sdk_version: str) -> Path:
        raise AssertionError("core sdk staging should not run for stage-sdk")

    def fake_stage_sdk_package(
        _staging_dir: Path, _sdk_version: str, _runtime_version: str
    ) -> Path:
        calls.append("stage_sdk")
        return tmp_path / "sdk-stage"

    def fake_stage_runtime_package(
        _staging_dir: Path, _runtime_version: str, _runtime_binary: Path
    ) -> Path:
        raise AssertionError("runtime staging should not run for stage-sdk")

    def fake_current_sdk_version() -> str:
        return "0.2.0"

    ops = script.CliOps(
        generate_types=fake_generate_types,
        stage_python_core_sdk_package=fake_stage_core_sdk_package,
        stage_python_sdk_package=fake_stage_sdk_package,
        stage_python_runtime_package=fake_stage_runtime_package,
        current_sdk_version=fake_current_sdk_version,
    )

    script.run_command(args, ops)

    assert calls == ["generate_types", "stage_sdk"]


def test_stage_sdk_can_skip_type_generation(tmp_path: Path) -> None:
    script = _load_update_script_module()
    calls: list[str] = []
    args = script.parse_args(
        [
            "stage-sdk",
            str(tmp_path / "sdk-stage"),
            "--runtime-version",
            "1.2.3",
            "--skip-generate-types",
        ]
    )

    def fake_generate_types() -> None:
        calls.append("generate_types")

    def fake_stage_core_sdk_package(_staging_dir: Path, _sdk_version: str) -> Path:
        raise AssertionError("core sdk staging should not run for stage-sdk")

    def fake_stage_sdk_package(
        _staging_dir: Path, _sdk_version: str, _runtime_version: str
    ) -> Path:
        calls.append("stage_sdk")
        return tmp_path / "sdk-stage"

    def fake_stage_runtime_package(
        _staging_dir: Path, _runtime_version: str, _runtime_binary: Path
    ) -> Path:
        raise AssertionError("runtime staging should not run for stage-sdk")

    def fake_current_sdk_version() -> str:
        return "0.2.0"

    ops = script.CliOps(
        generate_types=fake_generate_types,
        stage_python_core_sdk_package=fake_stage_core_sdk_package,
        stage_python_sdk_package=fake_stage_sdk_package,
        stage_python_runtime_package=fake_stage_runtime_package,
        current_sdk_version=fake_current_sdk_version,
    )

    script.run_command(args, ops)

    assert calls == ["stage_sdk"]


def test_stage_runtime_stages_binary_without_type_generation(tmp_path: Path) -> None:
    script = _load_update_script_module()
    fake_binary = tmp_path / script.runtime_binary_name()
    fake_binary.write_text("fake codex\n")
    calls: list[str] = []
    args = script.parse_args(
        [
            "stage-runtime",
            str(tmp_path / "runtime-stage"),
            str(fake_binary),
            "--runtime-version",
            "1.2.3",
        ]
    )

    def fake_generate_types() -> None:
        calls.append("generate_types")

    def fake_stage_core_sdk_package(_staging_dir: Path, _sdk_version: str) -> Path:
        raise AssertionError("core sdk staging should not run for stage-runtime")

    def fake_stage_sdk_package(
        _staging_dir: Path, _sdk_version: str, _runtime_version: str
    ) -> Path:
        raise AssertionError("sdk staging should not run for stage-runtime")

    def fake_stage_runtime_package(
        _staging_dir: Path, _runtime_version: str, _runtime_binary: Path
    ) -> Path:
        calls.append("stage_runtime")
        return tmp_path / "runtime-stage"

    def fake_current_sdk_version() -> str:
        return "0.2.0"

    ops = script.CliOps(
        generate_types=fake_generate_types,
        stage_python_core_sdk_package=fake_stage_core_sdk_package,
        stage_python_sdk_package=fake_stage_sdk_package,
        stage_python_runtime_package=fake_stage_runtime_package,
        current_sdk_version=fake_current_sdk_version,
    )

    script.run_command(args, ops)

    assert calls == ["stage_runtime"]


def test_default_runtime_is_resolved_from_installed_runtime_package(
    tmp_path: Path,
) -> None:
    from codex_app_server import client as client_module

    fake_binary = tmp_path / ("codex.exe" if client_module.os.name == "nt" else "codex")
    fake_binary.write_text("")
    ops = client_module.CodexBinResolverOps(
        installed_codex_path=lambda: fake_binary,
        path_exists=lambda path: path == fake_binary,
    )

    config = client_module.AppServerConfig()
    assert config.codex_bin is None
    assert client_module.resolve_codex_bin(config, ops) == fake_binary


def test_explicit_codex_bin_override_takes_priority(tmp_path: Path) -> None:
    from codex_app_server import client as client_module

    explicit_binary = tmp_path / (
        "custom-codex.exe" if client_module.os.name == "nt" else "custom-codex"
    )
    explicit_binary.write_text("")
    ops = client_module.CodexBinResolverOps(
        installed_codex_path=lambda: (_ for _ in ()).throw(
            AssertionError("packaged runtime should not be used")
        ),
        path_exists=lambda path: path == explicit_binary,
    )

    config = client_module.AppServerConfig(codex_bin=str(explicit_binary))
    assert client_module.resolve_codex_bin(config, ops) == explicit_binary


def test_missing_runtime_package_requires_explicit_codex_bin() -> None:
    from codex_app_server import client as client_module

    ops = client_module.CodexBinResolverOps(
        installed_codex_path=lambda: (_ for _ in ()).throw(
            FileNotFoundError("missing packaged runtime")
        ),
        path_exists=lambda _path: False,
    )

    with pytest.raises(FileNotFoundError, match="missing packaged runtime"):
        client_module.resolve_codex_bin(client_module.AppServerConfig(), ops)


def test_broken_runtime_package_does_not_fall_back() -> None:
    from codex_app_server import client as client_module

    ops = client_module.CodexBinResolverOps(
        installed_codex_path=lambda: (_ for _ in ()).throw(
            FileNotFoundError("missing packaged binary")
        ),
        path_exists=lambda _path: False,
    )

    with pytest.raises(FileNotFoundError) as exc_info:
        client_module.resolve_codex_bin(client_module.AppServerConfig(), ops)

    assert str(exc_info.value) == ("missing packaged binary")
