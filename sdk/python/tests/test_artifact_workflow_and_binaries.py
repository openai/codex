from __future__ import annotations

import ast
import importlib.util
import io
import json
import sys
import tomllib
import urllib.error
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


def _load_runtime_setup_module():
    runtime_setup_path = ROOT / "_runtime_setup.py"
    spec = importlib.util.spec_from_file_location("_runtime_setup", runtime_setup_path)
    if spec is None or spec.loader is None:
        raise AssertionError(
            f"Failed to load runtime setup module: {runtime_setup_path}"
        )
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _load_runtime_package_module(package_root: Path):
    runtime_init = package_root / "src" / "codex_cli_bin" / "__init__.py"
    spec = importlib.util.spec_from_file_location(
        "codex_cli_bin_under_test", runtime_init
    )
    if spec is None or spec.loader is None:
        raise AssertionError(f"Failed to load runtime package module: {runtime_init}")
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

    calls = {
        node.func.id
        for node in ast.walk(generate_types_fn)
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)
    }

    assert {
        "_generate_json_schema_from_runtime",
        "generate_v2_all",
        "generate_notification_registry",
        "generate_public_api_flat_methods",
    } <= calls


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


def test_examples_readme_matches_pinned_runtime_version() -> None:
    runtime_setup = _load_runtime_setup_module()
    readme = (ROOT / "examples" / "README.md").read_text()
    assert (
        f"Current pinned runtime version: `{runtime_setup.pinned_runtime_version()}`"
        in readme
    )


def test_release_metadata_retries_without_invalid_auth(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    runtime_setup = _load_runtime_setup_module()
    authorizations: list[str | None] = []

    def fake_urlopen(request):
        authorization = request.headers.get("Authorization")
        authorizations.append(authorization)
        if authorization is not None:
            raise urllib.error.HTTPError(
                request.full_url,
                401,
                "Unauthorized",
                hdrs=None,
                fp=None,
            )
        return io.StringIO('{"assets": []}')

    monkeypatch.setenv("GH_TOKEN", "invalid-token")
    monkeypatch.setattr(runtime_setup.urllib.request, "urlopen", fake_urlopen)

    assert runtime_setup._release_metadata("1.2.3") == {"assets": []}
    assert authorizations == ["Bearer invalid-token", None]


def test_runtime_package_is_wheel_only_and_builds_platform_specific_wheels() -> None:
    pyproject = tomllib.loads(
        (ROOT.parent / "python-runtime" / "pyproject.toml").read_text()
    )
    hook_source = (ROOT.parent / "python-runtime" / "hatch_build.py").read_text()
    hook_tree = ast.parse(hook_source)
    platform_tag_assignment = next(
        node
        for node in hook_tree.body
        if isinstance(node, ast.Assign)
        and len(node.targets) == 1
        and isinstance(node.targets[0], ast.Name)
        and node.targets[0].id == "PLATFORM_TAG_BY_TARGET"
    )
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

    assert pyproject["project"]["name"] == "openai-codex-cli-bin"
    assert pyproject["tool"]["hatch"]["build"]["targets"]["wheel"] == {
        "packages": ["src/codex_cli_bin"],
        "include": ["src/codex_cli_bin/bin/**"],
        "hooks": {"custom": {}},
    }
    assert pyproject["tool"]["hatch"]["build"]["targets"]["sdist"] == {
        "hooks": {"custom": {}},
    }
    assert sdist_guard is not None
    assert build_data_assignments == {"pure_python": False}
    assert ast.literal_eval(platform_tag_assignment.value) == {
        "aarch64-apple-darwin": "macosx_11_0_arm64",
        "x86_64-apple-darwin": "macosx_10_12_x86_64",
        "aarch64-unknown-linux-musl": "musllinux_1_2_aarch64",
        "x86_64-unknown-linux-musl": "musllinux_1_2_x86_64",
        "aarch64-pc-windows-msvc": "win_arm64",
        "x86_64-pc-windows-msvc": "win_amd64",
    }
    assert "CODEX_PYTHON_RUNTIME_TARGET" in hook_source
    assert '"infer_tag"' in hook_source
    assert '"tag"' in hook_source


def test_python_release_version_normalization() -> None:
    script = _load_update_script_module()

    assert script.normalize_python_package_version("1.2.3") == "1.2.3"
    assert script.normalize_python_package_version("1.2.3-alpha.4") == "1.2.3a4"
    assert script.normalize_python_package_version("1.2.3-beta.5") == "1.2.3b5"
    assert script.normalize_python_package_version("1.2.3a4") == "1.2.3a4"
    assert script.normalize_python_package_version("0.0.0.dev0") == "0.0.0.dev0"

    with pytest.raises(RuntimeError, match="Unsupported Python package version"):
        script.normalize_python_package_version("1.2.3-rc.1")


def test_stage_runtime_release_copies_bundle_and_sets_version(tmp_path: Path) -> None:
    script = _load_update_script_module()
    bundle_dir = tmp_path / "bundle"
    bundle_dir.mkdir()
    fake_binary = bundle_dir / script.runtime_binary_name()
    fake_binary.write_text("fake codex\n")

    staged = script.stage_python_runtime_package(
        tmp_path / "runtime-stage",
        "1.2.3-alpha.4",
        bundle_dir,
    )

    assert staged == tmp_path / "runtime-stage"
    assert script.staged_runtime_bin_path(staged).read_text() == "fake codex\n"
    pyproject = (staged / "pyproject.toml").read_text()
    assert 'name = "openai-codex-cli-bin"' in pyproject
    assert 'version = "1.2.3a4"' in pyproject


def test_stage_runtime_release_replaces_existing_staging_dir(tmp_path: Path) -> None:
    script = _load_update_script_module()
    staging_dir = tmp_path / "runtime-stage"
    old_file = staging_dir / "stale.txt"
    old_file.parent.mkdir(parents=True)
    old_file.write_text("stale")

    bundle_dir = tmp_path / "bundle"
    bundle_dir.mkdir()
    fake_binary = bundle_dir / script.runtime_binary_name()
    fake_binary.write_text("fake codex\n")

    staged = script.stage_python_runtime_package(
        staging_dir,
        "1.2.3",
        bundle_dir,
    )

    assert staged == staging_dir
    assert not old_file.exists()
    assert script.staged_runtime_bin_path(staged).read_text() == "fake codex\n"


def test_stage_runtime_release_normalizes_target_suffixed_names(
    tmp_path: Path,
) -> None:
    script = _load_update_script_module()
    bundle_dir = tmp_path / "bundle"
    bundle_dir.mkdir()
    (bundle_dir / "codex-x86_64-unknown-linux-musl").write_text("fake codex\n")

    staged = script.stage_python_runtime_package(
        tmp_path / "runtime-stage",
        "1.2.3",
        bundle_dir,
    )

    assert (staged / "src" / "codex_cli_bin" / "bin" / "codex").read_text() == (
        "fake codex\n"
    )


def test_stage_runtime_release_requires_complete_windows_bundle(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    script = _load_update_script_module()
    monkeypatch.setattr(script.platform, "system", lambda: "Windows")
    bundle_dir = tmp_path / "bundle"
    bundle_dir.mkdir()
    (bundle_dir / "codex-x86_64-pc-windows-msvc.exe").write_text("codex\n")
    (bundle_dir / "codex-command-runner-x86_64-pc-windows-msvc.exe").write_text(
        "runner\n"
    )
    (bundle_dir / "codex-windows-sandbox-setup-x86_64-pc-windows-msvc.exe").write_text(
        "setup\n"
    )

    staged = script.stage_python_runtime_package(
        tmp_path / "runtime-stage",
        "1.2.3",
        bundle_dir,
    )
    bin_dir = staged / "src" / "codex_cli_bin" / "bin"

    assert (bin_dir / "codex.exe").read_text() == "codex\n"
    assert (bin_dir / "codex-command-runner.exe").read_text() == "runner\n"
    assert (bin_dir / "codex-windows-sandbox-setup.exe").read_text() == "setup\n"


def test_stage_runtime_release_fails_for_missing_required_file(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    script = _load_update_script_module()
    monkeypatch.setattr(script.platform, "system", lambda: "Windows")
    bundle_dir = tmp_path / "bundle"
    bundle_dir.mkdir()
    (bundle_dir / "codex.exe").write_text("codex\n")

    with pytest.raises(RuntimeError, match="codex-command-runner.exe"):
        script.stage_python_runtime_package(
            tmp_path / "runtime-stage",
            "1.2.3",
            bundle_dir,
        )


def test_runtime_package_helpers_return_packaged_paths(tmp_path: Path) -> None:
    script = _load_update_script_module()
    bundle_dir = tmp_path / "bundle"
    bundle_dir.mkdir()
    (bundle_dir / "codex").write_text("fake codex\n")
    staged = script.stage_python_runtime_package(
        tmp_path / "runtime-stage",
        "1.2.3",
        bundle_dir,
    )

    runtime_module = _load_runtime_package_module(staged)

    assert runtime_module.PACKAGE_NAME == "openai-codex-cli-bin"
    assert runtime_module.bundled_bin_dir() == staged / "src" / "codex_cli_bin" / "bin"
    assert runtime_module.bundled_runtime_files() == (
        staged / "src" / "codex_cli_bin" / "bin" / "codex",
    )
    assert runtime_module.bundled_codex_path() == (
        staged / "src" / "codex_cli_bin" / "bin" / "codex"
    )


def test_runtime_package_helpers_report_missing_binary(tmp_path: Path) -> None:
    script = _load_update_script_module()
    bundle_dir = tmp_path / "bundle"
    bundle_dir.mkdir()
    (bundle_dir / "codex").write_text("fake codex\n")
    staged = script.stage_python_runtime_package(
        tmp_path / "runtime-stage",
        "1.2.3",
        bundle_dir,
    )
    (staged / "src" / "codex_cli_bin" / "bin" / "codex").unlink()

    runtime_module = _load_runtime_package_module(staged)

    with pytest.raises(FileNotFoundError, match="openai-codex-cli-bin"):
        runtime_module.bundled_codex_path()


def test_stage_sdk_release_injects_exact_runtime_pin_and_versions(
    tmp_path: Path,
) -> None:
    script = _load_update_script_module()
    staged = script.stage_python_sdk_package(
        tmp_path / "sdk-stage",
        "0.2.1-beta.2",
        "1.2.3-alpha.4",
    )

    pyproject = (staged / "pyproject.toml").read_text()
    assert 'name = "openai-codex"' in pyproject
    assert 'version = "0.2.1b2"' in pyproject
    assert '"openai-codex-cli-bin==1.2.3a4"' in pyproject
    assert (
        '__version__ = "0.2.1b2"'
        in (staged / "src" / "codex_app_server" / "__init__.py").read_text()
    )
    assert (
        'client_version: str = "0.2.1b2"'
        in (staged / "src" / "codex_app_server" / "client.py").read_text()
    )
    assert not any((staged / "src" / "codex_app_server").glob("bin/**"))


def test_stage_sdk_release_replaces_existing_staging_dir(tmp_path: Path) -> None:
    script = _load_update_script_module()
    staging_dir = tmp_path / "sdk-stage"
    old_file = staging_dir / "stale.txt"
    old_file.parent.mkdir(parents=True)
    old_file.write_text("stale")

    staged = script.stage_python_sdk_package(staging_dir, "0.2.1", "1.2.3")

    assert staged == staging_dir
    assert not old_file.exists()


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

    def fake_generate_types(runtime_version: str | None) -> None:
        calls.append(f"generate_types:{runtime_version}")

    def fake_stage_sdk_package(
        _staging_dir: Path, _sdk_version: str, _runtime_version: str
    ) -> Path:
        calls.append("stage_sdk")
        return tmp_path / "sdk-stage"

    def fake_stage_runtime_package(
        _staging_dir: Path, _runtime_version: str, _runtime_bundle_dir: Path
    ) -> Path:
        raise AssertionError("runtime staging should not run for stage-sdk")

    def fake_current_sdk_version() -> str:
        return "0.2.0"

    ops = script.CliOps(
        generate_types=fake_generate_types,
        stage_python_sdk_package=fake_stage_sdk_package,
        stage_python_runtime_package=fake_stage_runtime_package,
        current_sdk_version=fake_current_sdk_version,
    )

    script.run_command(args, ops)

    assert calls == ["generate_types:None", "stage_sdk"]


def test_generate_types_accepts_runtime_version_override() -> None:
    script = _load_update_script_module()
    calls: list[str] = []
    args = script.parse_args(
        [
            "generate-types",
            "--runtime-version",
            "1.2.3-alpha.4",
        ]
    )

    def fake_generate_types(runtime_version: str | None) -> None:
        calls.append(f"generate_types:{runtime_version}")

    def fake_stage_sdk_package(
        _staging_dir: Path, _sdk_version: str, _runtime_version: str
    ) -> Path:
        raise AssertionError("sdk staging should not run for generate-types")

    def fake_stage_runtime_package(
        _staging_dir: Path, _runtime_version: str, _runtime_bundle_dir: Path
    ) -> Path:
        raise AssertionError("runtime staging should not run for generate-types")

    def fake_current_sdk_version() -> str:
        return "0.2.0"

    ops = script.CliOps(
        generate_types=fake_generate_types,
        stage_python_sdk_package=fake_stage_sdk_package,
        stage_python_runtime_package=fake_stage_runtime_package,
        current_sdk_version=fake_current_sdk_version,
    )

    script.run_command(args, ops)

    assert calls == ["generate_types:1.2.3-alpha.4"]


def test_runtime_schema_generator_uses_app_server_json_schema_command(
    tmp_path: Path,
) -> None:
    script = _load_update_script_module()
    codex_bin = tmp_path / "codex"
    out_dir = tmp_path / "schema"
    args_path = tmp_path / "args.txt"
    codex_bin.write_text(
        "#!/usr/bin/env sh\n"
        f'printf \'%s\\n\' "$@" > "{args_path}"\n'
        'mkdir -p "$4"\n'
        "printf '{}' > \"$4/codex_app_server_protocol.v2.schemas.json\"\n"
        "printf '{}' > \"$4/ServerNotification.json\"\n"
    )
    codex_bin.chmod(0o755)

    script._run_runtime_schema_generator(codex_bin, out_dir)

    assert args_path.read_text().splitlines() == [
        "app-server",
        "generate-json-schema",
        "--out",
        str(out_dir),
    ]


def test_stage_runtime_stages_binary_without_type_generation(tmp_path: Path) -> None:
    script = _load_update_script_module()
    bundle_dir = tmp_path / "bundle"
    bundle_dir.mkdir()
    (bundle_dir / script.runtime_binary_name()).write_text("fake codex\n")
    calls: list[str] = []
    args = script.parse_args(
        [
            "stage-runtime",
            str(tmp_path / "runtime-stage"),
            str(bundle_dir),
            "--runtime-version",
            "1.2.3",
        ]
    )

    def fake_generate_types(_runtime_version: str | None) -> None:
        calls.append("generate_types")

    def fake_stage_sdk_package(
        _staging_dir: Path, _sdk_version: str, _runtime_version: str
    ) -> Path:
        raise AssertionError("sdk staging should not run for stage-runtime")

    def fake_stage_runtime_package(
        _staging_dir: Path, _runtime_version: str, _runtime_bundle_dir: Path
    ) -> Path:
        calls.append("stage_runtime")
        return tmp_path / "runtime-stage"

    def fake_current_sdk_version() -> str:
        return "0.2.0"

    ops = script.CliOps(
        generate_types=fake_generate_types,
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
