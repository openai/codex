import ast
import importlib.util
import io
import json
import os
import re
import subprocess
import sys
import tarfile
import urllib.error
import zipfile
from email.parser import BytesParser
from pathlib import Path

import pytest
from pydantic import ValidationError

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

ROOT = Path(__file__).resolve().parents[1]


def _load_root_format_script_module():
    """Load the root formatter driver so tests exercise its real command graph."""
    script_path = ROOT.parents[1] / "scripts" / "format.py"
    spec = importlib.util.spec_from_file_location("format_repo", script_path)
    if spec is None or spec.loader is None:
        raise AssertionError(f"Failed to load script module: {script_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _load_update_script_module():
    """Load the maintenance script as a module so tests exercise real helpers."""
    script_path = ROOT / "scripts" / "update_sdk_artifacts.py"
    spec = importlib.util.spec_from_file_location("update_sdk_artifacts", script_path)
    if spec is None or spec.loader is None:
        raise AssertionError(f"Failed to load script module: {script_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _load_runtime_setup_module():
    """Load runtime setup without importing the SDK package under test."""
    runtime_setup_path = ROOT / "_runtime_setup.py"
    spec = importlib.util.spec_from_file_location("_runtime_setup", runtime_setup_path)
    if spec is None or spec.loader is None:
        raise AssertionError(f"Failed to load runtime setup module: {runtime_setup_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _load_release_version_module():
    """Load the shared release-version conversions used by release tooling."""
    script_path = ROOT / "release_version.py"
    spec = importlib.util.spec_from_file_location("release_version", script_path)
    if spec is None or spec.loader is None:
        raise AssertionError(f"Failed to load release-version module: {script_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _write_fake_codex_package(package_dir: Path, script) -> Path:
    (package_dir / "bin").mkdir(parents=True)
    (package_dir / "codex-resources").mkdir()
    (package_dir / "codex-path").mkdir()
    (package_dir / "codex-package.json").write_text('{"variant":"codex"}\n')
    (package_dir / "bin" / script.runtime_binary_name()).write_text("fake codex\n")
    (package_dir / "bin" / script.runtime_code_mode_host_name()).write_text("fake code mode host\n")
    (package_dir / "codex-resources" / "bwrap").write_text("fake bwrap\n")
    (package_dir / "codex-path" / "rg").write_text("fake rg\n")
    return package_dir


def _write_fake_codex_package_archive(tmp_path: Path, script) -> Path:
    package_dir = _write_fake_codex_package(tmp_path / "codex-package", script)
    archive_path = tmp_path / "codex-package.tar.gz"
    _write_package_archive(package_dir, archive_path)
    return archive_path


def _write_package_archive(package_dir: Path, archive_path: Path) -> None:
    with tarfile.open(archive_path, "w:gz") as archive:
        for path in package_dir.rglob("*"):
            archive.add(path, arcname=path.relative_to(package_dir))


def test_generation_has_single_maintenance_entrypoint_script() -> None:
    """Keep artifact workflows routed through one script instead of side entrypoints."""
    scripts = sorted(p.name for p in (ROOT / "scripts").glob("*.py"))
    assert scripts == ["update_sdk_artifacts.py"]


def test_root_fmt_recipes_use_shared_formatter_driver() -> None:
    """The root formatting recipes should use the shared cross-platform driver."""
    justfile = ROOT.parents[1] / "justfile"
    lines = justfile.read_text().splitlines()
    fmt_index = lines.index("fmt:")
    fmt_check_index = lines.index("fmt-check:")
    next_recipe_index = next(
        index
        for index in range(fmt_check_index + 1, len(lines))
        if lines[index] and not lines[index].startswith((" ", "\t", "#"))
    )
    actual = {
        "working_directory": lines[0],
        "fmt_comment": next(line for line in reversed(lines[:fmt_index]) if line.startswith("#")),
        "fmt_commands": [
            line.strip()
            for line in lines[fmt_index + 1 : fmt_check_index]
            if line.strip() and not line.startswith("#")
        ],
        "fmt_check_comment": next(
            line for line in reversed(lines[:fmt_check_index]) if line.startswith("#")
        ),
        "fmt_check_commands": [
            line.strip() for line in lines[fmt_check_index + 1 : next_recipe_index] if line.strip()
        ],
    }
    expected = {
        "working_directory": 'set working-directory := "codex-rs"',
        "fmt_comment": (
            "# Format the justfile, Rust, Bazel/Starlark, Python SDK code, and Python scripts."
        ),
        "fmt_commands": ["@{{ python }} ../scripts/format.py"],
        "fmt_check_comment": "# Check formatting without modifying files.",
        "fmt_check_commands": ["@{{ python }} ../scripts/format.py --check"],
    }

    assert actual == expected, (
        "The root formatting recipes must use the shared formatter driver. "
        "Fix the recipes in `justfile`, then run `just fmt`.\n"
        f"Expected: {json.dumps(expected, indent=2)}\n"
        f"Actual: {json.dumps(actual, indent=2)}"
    )


def test_root_format_driver_covers_all_formatter_groups(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """The shared driver should retain every formatter in both modes."""
    script = _load_root_format_script_module()
    for name in (
        "bazel/rules/example.rs",
        "codex-rs/src/lib.rs",
        "codex-rs/new file.rs",
    ):
        path = tmp_path / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("")
    git_ls_files_args = [
        "git",
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
    ]

    # The Python SDK CI image has no Git; keep discovery mocked at the process boundary.
    def fake_check_output(args, *, cwd):
        assert cwd == tmp_path
        if args == git_ls_files_args + ["--", "*.rs"]:
            return (
                b"codex-rs/src/lib.rs\0bazel/rules/example.rs\0"
                b"codex-rs/new file.rs\0codex-rs/deleted.rs\0"
            )
        assert args == git_ls_files_args
        return b"MODULE.bazel\0README.md\0third_party/v8/libcxx.BUILD.bazel\0"

    monkeypatch.setattr(script, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(script.subprocess, "check_output", fake_check_output)
    formatters = script.formatter_groups(check=False)
    checks = script.formatter_groups(check=True)

    assert [group.name for group in formatters] == [
        "Just",
        "Rust",
        "Bazel/Starlark",
        "Python SDK",
        "Python scripts",
    ]
    assert [group.name for group in checks] == [group.name for group in formatters]
    assert [len(group.commands) for group in formatters] == [1, 1, 1, 2, 1]
    assert [len(group.commands) for group in checks] == [
        len(group.commands) for group in formatters
    ]
    sdk_uv_run_args = (
        "uv",
        "run",
        "--frozen",
        "--project",
        "sdk/python",
        "--only-group",
        "format",
    )
    scripts_uv_run_args = (
        "uv",
        "run",
        "--frozen",
        "--project",
        "scripts",
    )
    assert all(
        command.args[: len(sdk_uv_run_args)] == sdk_uv_run_args
        for group in (formatters[3], checks[3])
        for command in group.commands
    )
    assert all(
        command.args[: len(scripts_uv_run_args)] == scripts_uv_run_args
        for group in (formatters[4], checks[4])
        for command in group.commands
    )
    assert formatters[3].commands[0].args[-5:] == (
        "ruff",
        "check",
        "--fix",
        "--fix-only",
        "sdk/python",
    )
    assert checks[3].commands[0].args[-4:] == (
        "ruff",
        "check",
        "--diff",
        "sdk/python",
    )
    assert formatters[0].commands[-1].args == ("just", "--unstable", "--fmt")
    assert checks[0].commands[-1].args == ("just", "--unstable", "--fmt", "--check")
    rustfmt_args = (
        "rustfmt",
        "--edition",
        "2024",
        "--config-path",
        str(tmp_path / "codex-rs/rustfmt.toml"),
        "--config",
        "imports_granularity=Item,skip_children=true",
    )
    rust_files = (
        os.path.join("..", "bazel", "rules", "example.rs"),
        "new file.rs",
        os.path.join("src", "lib.rs"),
    )
    assert formatters[1].commands == (
        script.Command(rustfmt_args + rust_files, tmp_path / "codex-rs"),
    )
    assert checks[1].commands == (
        script.Command(rustfmt_args + ("--check",) + rust_files, tmp_path / "codex-rs"),
    )
    format_buildifier_args = formatters[2].commands[-1].args
    check_buildifier_args = checks[2].commands[-1].args
    assert format_buildifier_args[:4] == (
        "dotslash",
        str(script.REPO_ROOT / "tools" / "buildifier"),
        "-mode=fix",
        "-lint=off",
    )
    assert check_buildifier_args[:4] == (
        "dotslash",
        str(script.REPO_ROOT / "tools" / "buildifier"),
        "-mode=check",
        "-lint=off",
    )
    assert format_buildifier_args[4:] == check_buildifier_args[4:]
    assert format_buildifier_args[4:] == (
        "MODULE.bazel",
        "third_party/v8/libcxx.BUILD.bazel",
    )
    assert [group.commands[-1].args[-3:] for group in formatters[3:]] == [
        ("ruff", "format", "sdk/python"),
        ("ruff", "format", "."),
    ]
    assert [group.commands[-1].args[-4:] for group in checks[3:]] == [
        ("ruff", "format", "--check", "sdk/python"),
        ("ruff", "format", "--check", "."),
    ]


def test_root_format_driver_discards_successful_command_output(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    script = _load_root_format_script_module()
    processes = iter(
        (
            script.subprocess.CompletedProcess(("first",), 0, "routine output\n"),
            script.subprocess.CompletedProcess(("second",), 2, "failure output\n"),
        )
    )
    monkeypatch.setattr(script.subprocess, "run", lambda *args, **kwargs: next(processes))
    group = script.FormatterGroup(
        "Test",
        (script.Command(("first",)), script.Command(("second",))),
    )

    assert script.run_formatter_group(group) == script.FormatterResult(
        "Test",
        "$ second\nfailure output\n",
        2,
    )


def test_root_format_driver_is_silent_when_all_formatters_succeed(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    script = _load_root_format_script_module()
    groups = (script.FormatterGroup("Quiet", ()),)
    monkeypatch.setattr(script, "formatter_groups", lambda *, check: groups)
    monkeypatch.setattr(
        script,
        "run_formatter_group",
        lambda group: script.FormatterResult(group.name, "hidden output\n", 0),
    )
    monkeypatch.setattr(sys, "argv", ["format.py"])

    assert script.main() == 0
    captured = capsys.readouterr()
    assert (captured.out, captured.err) == ("", "")


def test_root_format_driver_reports_only_failed_formatters(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    script = _load_root_format_script_module()
    groups = (
        script.FormatterGroup("Quiet", ()),
        script.FormatterGroup("Broken", ()),
    )
    monkeypatch.setattr(script, "formatter_groups", lambda *, check: groups)

    def fake_run(group):
        if group.name == "Broken":
            return script.FormatterResult(group.name, "$ broken\nfailure output\n", 2)
        return script.FormatterResult(group.name, "hidden output\n", 0)

    monkeypatch.setattr(script, "run_formatter_group", fake_run)
    monkeypatch.setattr(sys, "argv", ["format.py"])

    assert script.main() == 1
    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == (
        "==> Broken formatter failed\n$ broken\nfailure output\nFormatting failed: Broken\n"
    )


def test_generate_types_wires_all_generation_steps() -> None:
    """The type generation command should refresh every schema-derived artifact."""
    source = (ROOT / "scripts" / "update_sdk_artifacts.py").read_text()
    tree = ast.parse(source)

    generate_types_fn = next(
        (
            node
            for node in tree.body
            if isinstance(node, ast.FunctionDef) and node.name == "generate_types_from_schema_dir"
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


@pytest.mark.parametrize("schema_override", [None, "override-schema"])
def test_generation_resolves_configured_schema_and_explicit_override(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, schema_override: str | None
) -> None:
    script = _load_update_script_module()
    sdk_dir = tmp_path / "sdk" / "python"
    sdk_dir.mkdir(parents=True)
    monkeypatch.setattr(script, "sdk_root", lambda: sdk_dir)
    monkeypatch.chdir(tmp_path)
    selected_schemas: list[Path] = []
    monkeypatch.setattr(script, "generate_types_from_schema_dir", selected_schemas.append)
    args = ["generate-types"]
    if schema_override is None:
        (sdk_dir / "pyproject.toml").write_text(
            '[tool.codex.codegen]\nschema-dir = "../../configured-schema"\n'
        )
        expected_schema = tmp_path / "configured-schema"
    else:
        args.extend(["--schema-dir", schema_override])
        expected_schema = tmp_path / schema_override

    script.main(args)

    assert selected_schemas == [expected_schema]


def _load_repository_schema_bundle() -> dict:
    """Read the repository app-server schema bundle used by generation."""
    script = _load_update_script_module()
    pyproject = tomllib.loads((ROOT / "pyproject.toml").read_text())
    schema_dir = ROOT / pyproject["tool"]["codex"]["codegen"]["schema-dir"]
    return json.loads(script.schema_bundle_path(schema_dir).read_text())


def test_schema_normalization_flattens_string_literal_oneofs() -> None:
    script = _load_update_script_module()
    definition = {
        "title": "Mode",
        "description": "Allowed modes.",
        "oneOf": [
            {"type": "string", "enum": ["first"]},
            {"type": "string", "enum": ["second"]},
        ],
    }

    assert script._flatten_string_enum_one_of(definition)
    assert definition == {
        "title": "Mode",
        "description": "Allowed modes.",
        "type": "string",
        "enum": ["first", "second"],
    }


@pytest.mark.parametrize(
    "branch",
    [
        {"type": "object", "properties": {"value": {"type": "string"}}},
        {"type": "string", "enum": ["first", "second"]},
        {"type": "string", "enum": [1]},
        {"type": "string", "enum": ["second"], "minLength": 2},
    ],
)
def test_schema_normalization_preserves_nonliteral_unions(branch: dict) -> None:
    script = _load_update_script_module()
    definition = {"oneOf": [{"type": "string", "enum": ["first"]}, branch]}
    original = json.loads(json.dumps(definition))

    assert not script._flatten_string_enum_one_of(definition)
    assert definition == original


def test_schema_normalization_makes_chatgpt_account_email_nullable() -> None:
    script = _load_update_script_module()
    schema = {
        "definitions": {
            "Account": {
                "oneOf": [
                    {
                        "properties": {
                            "email": {"type": "string"},
                            "type": {"enum": ["chatgpt"], "type": "string"},
                        },
                        "required": ["email", "type"],
                        "type": "object",
                    }
                ]
            }
        }
    }

    script._make_chatgpt_account_email_nullable(schema)

    chatgpt_account = schema["definitions"]["Account"]["oneOf"][0]
    assert chatgpt_account["properties"]["email"]["type"] == ["string", "null"]
    assert "email" in chatgpt_account["required"]


def test_python_codegen_schema_annotation_adds_stable_variant_titles() -> None:
    """Schema annotations should give generated protocol classes stable names."""
    script = _load_update_script_module()
    schema = _load_repository_schema_bundle()
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


def test_generated_chatgpt_account_email_is_required_nullable() -> None:
    from openai_codex.generated.v2_all import ChatgptAccount

    account = ChatgptAccount.model_validate({"email": None, "planType": "pro", "type": "chatgpt"})
    assert account.email is None
    assert ChatgptAccount.model_fields["email"].is_required()

    with pytest.raises(ValidationError):
        ChatgptAccount.model_validate({"planType": "pro", "type": "chatgpt"})


def test_generated_inline_image_class_names_remain_stable() -> None:
    """Keep the existing Python class names when image references expand."""
    from openai_codex.generated.v2_all import (
        ImageUserInput,
        InputImageContentItem,
        InputImageFunctionCallOutputContentItem,
    )

    assert ImageUserInput.__name__ == "ImageUserInput"
    assert InputImageContentItem.__name__ == "InputImageContentItem"
    assert (
        InputImageFunctionCallOutputContentItem.__name__
        == "InputImageFunctionCallOutputContentItem"
    )


def test_runtime_package_template_has_no_checked_in_binaries() -> None:
    runtime_root = ROOT.parent / "python-runtime" / "src" / "codex_cli_bin"
    assert sorted(
        path.name
        for path in runtime_root.rglob("*")
        if path.is_file() and "__pycache__" not in path.parts
    ) == ["__init__.py"]


def test_examples_readme_points_to_runtime_version_source_of_truth() -> None:
    """Document that examples should point at the dependency pin, not release lore."""
    readme = (ROOT / "examples" / "README.md").read_text()
    assert "The pinned runtime version comes from the SDK package dependency." in readme


def test_runtime_distribution_name_is_consistent() -> None:
    script = _load_update_script_module()
    runtime_setup = _load_runtime_setup_module()
    from openai_codex import _version, client as client_module

    assert script.SDK_DISTRIBUTION_NAME == "openai-codex"
    assert runtime_setup.SDK_PACKAGE_NAME == "openai-codex"
    assert _version.DISTRIBUTION_NAME == "openai-codex"
    assert script.RUNTIME_DISTRIBUTION_NAME == "openai-codex-cli-bin"
    assert runtime_setup.PACKAGE_NAME == "openai-codex-cli-bin"
    assert client_module.RUNTIME_PKG_NAME == "openai-codex-cli-bin"
    assert (
        "importlib.metadata.version('codex-cli-bin')"
        not in (ROOT / "_runtime_setup.py").read_text()
    )


def test_source_sdk_package_declares_stable_documentation() -> None:
    """Public package metadata should link stable docs."""
    pyproject = tomllib.loads((ROOT / "pyproject.toml").read_text())
    readme = (ROOT / "README.md").read_text()

    assert {
        "description": pyproject["project"]["description"],
        "is_stable": "Development Status :: 5 - Production/Stable"
        in pyproject["project"]["classifiers"],
        "license": pyproject["project"]["license"],
        "documentation": pyproject["project"]["urls"]["Documentation"],
        "readme_is_stable": "# OpenAI Codex Python SDK\n" in readme,
        "local_license_file": (ROOT / "LICENSE").exists(),
    } == {
        "description": "Python SDK for Codex",
        "is_stable": True,
        "license": "Apache-2.0",
        "documentation": "https://github.com/openai/codex/tree/main/sdk/python/docs",
        "readme_is_stable": True,
        "local_license_file": False,
    }


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


def test_runtime_setup_reads_independent_runtime_pin_and_release_tags() -> None:
    """Runtime package pins remain independent of the SDK template version."""
    runtime_setup = _load_runtime_setup_module()
    pyproject = tomllib.loads((ROOT / "pyproject.toml").read_text())

    assert {
        "package_name": runtime_setup.PACKAGE_NAME,
        "sdk_template_version": pyproject["project"]["version"],
        "runtime_pin": runtime_setup.pinned_runtime_version(),
        "normalized_release_version": runtime_setup._normalized_package_version(
            "rust-v0.116.0-alpha.1"
        ),
        "normalized_alpha_hotfix_version": runtime_setup._normalized_package_version(
            "rust-v0.116.0-alpha.1.2"
        ),
        "release_tag": runtime_setup._release_tag("0.116.0a1"),
        "alpha_hotfix_release_tag": runtime_setup._release_tag("0.116.0a1.post2"),
    } == {
        "package_name": "openai-codex-cli-bin",
        "sdk_template_version": "0.0.0-dev",
        "runtime_pin": "0.153.4",
        "normalized_release_version": "0.116.0a1",
        "normalized_alpha_hotfix_version": "0.116.0a1.post2",
        "release_tag": "rust-v0.116.0-alpha.1",
        "alpha_hotfix_release_tag": "rust-v0.116.0-alpha.1.2",
    }


@pytest.mark.parametrize(
    ("system", "machine", "asset_name"),
    [
        ("Darwin", "arm64", "codex-package-aarch64-apple-darwin.tar.gz"),
        ("Linux", "x86_64", "codex-package-x86_64-unknown-linux-musl.tar.gz"),
        ("Windows", "AMD64", "codex-package-x86_64-pc-windows-msvc.tar.gz"),
    ],
)
def test_runtime_setup_downloads_codex_package_archives(
    monkeypatch: pytest.MonkeyPatch,
    system: str,
    machine: str,
    asset_name: str,
) -> None:
    runtime_setup = _load_runtime_setup_module()
    monkeypatch.setattr(runtime_setup.platform, "system", lambda: system)
    monkeypatch.setattr(runtime_setup.platform, "machine", lambda: machine)

    assert runtime_setup.platform_asset_name() == asset_name


def test_runtime_package_is_wheel_only_and_builds_platform_specific_wheels() -> None:
    pyproject = tomllib.loads((ROOT.parent / "python-runtime" / "pyproject.toml").read_text())
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
    build_data_assignments = {}
    for node in initialize_fn.body:
        if (
            not isinstance(node, ast.Assign)
            or len(node.targets) != 1
            or not isinstance(node.targets[0], ast.Subscript)
            or not isinstance(node.targets[0].value, ast.Name)
            or node.targets[0].value.id != "build_data"
            or not isinstance(node.targets[0].slice, ast.Constant)
            or not isinstance(node.targets[0].slice.value, str)
        ):
            continue
        if isinstance(node.value, ast.Constant):
            build_data_assignments[node.targets[0].slice.value] = node.value.value
        elif isinstance(node.value, ast.JoinedStr):
            build_data_assignments[node.targets[0].slice.value] = "joined-string"

    assert pyproject["project"]["name"] == "openai-codex-cli-bin"
    assert pyproject["tool"]["hatch"]["build"]["targets"]["wheel"] == {
        "packages": ["src/codex_cli_bin"],
        "include": [
            "src/codex_cli_bin/codex-package.json",
            "src/codex_cli_bin/bin/**",
            "src/codex_cli_bin/codex-resources/**",
            "src/codex_cli_bin/codex-path/**",
        ],
        "hooks": {"custom": {}},
    }
    assert pyproject["tool"]["hatch"]["build"]["targets"]["sdist"] == {
        "hooks": {"custom": {}},
    }
    assert sdist_guard is not None
    assert build_data_assignments == {
        "pure_python": False,
        "infer_tag": False,
        "tag": "joined-string",
    }


def test_stage_runtime_release_copies_package_layout_and_sets_version(
    tmp_path: Path,
) -> None:
    script = _load_update_script_module()
    package_archive = _write_fake_codex_package_archive(tmp_path, script)

    staged = script.stage_python_runtime_package(
        tmp_path / "runtime-stage",
        "1.2.3",
        package_archive,
    )
    package_root = script.staged_runtime_package_root(staged)

    assert {
        "metadata": (package_root / "codex-package.json").read_text(),
        "codex": (package_root / "bin" / script.runtime_binary_name()).read_text(),
        "code_mode_host": (package_root / "bin" / script.runtime_code_mode_host_name()).read_text(),
        "bwrap": (package_root / "codex-resources" / "bwrap").read_text(),
        "rg": (package_root / "codex-path" / "rg").read_text(),
    } == {
        "metadata": '{"variant":"codex"}\n',
        "codex": "fake codex\n",
        "code_mode_host": "fake code mode host\n",
        "bwrap": "fake bwrap\n",
        "rg": "fake rg\n",
    }
    assert 'name = "openai-codex-cli-bin"' in (staged / "pyproject.toml").read_text()
    assert 'version = "1.2.3"' in (staged / "pyproject.toml").read_text()


def test_normalize_codex_version_accepts_release_tags_and_pep440_versions() -> None:
    script = _load_update_script_module()

    assert script.normalize_codex_version("rust-v0.116.0-alpha.1") == "0.116.0a1"
    assert script.normalize_codex_version("rust-v0.116.0-alpha.1.2") == "0.116.0a1.post2"
    assert script.normalize_codex_version("v0.116.0-beta.2") == "0.116.0b2"
    assert script.normalize_codex_version("0.116.0rc3") == "0.116.0rc3"
    assert script.normalize_codex_version("0.116.0") == "0.116.0"


def test_release_version_conversions_map_python_versions_to_codex_tags() -> None:
    release_version = _load_release_version_module()

    assert {
        version: release_version.codex_release_tag(version)
        for version in ["0.116.0", "0.116.0a1", "0.116.0a1.post2"]
    } == {
        "0.116.0": "rust-v0.116.0",
        "0.116.0a1": "rust-v0.116.0-alpha.1",
        "0.116.0a1.post2": "rust-v0.116.0-alpha.1.2",
    }


@pytest.mark.parametrize(
    ("version", "python_version", "release_tag"),
    [
        ("0.116.0a1.post2", "0.116.0a1.post2", "rust-v0.116.0-alpha.1.2"),
        ("rust-v1.2.3", "1.2.3", "rust-v1.2.3"),
        ("rust-v1.2.3-alpha.4", "1.2.3a4", "rust-v1.2.3-alpha.4"),
        ("rust-v1.2.3-alpha.4.5", "1.2.3a4.post5", "rust-v1.2.3-alpha.4.5"),
    ],
)
def test_release_version_cli_writes_python_runtime_outputs(
    tmp_path: Path,
    version: str,
    python_version: str,
    release_tag: str,
) -> None:
    github_output = tmp_path / "github-output"

    result = subprocess.run(
        [
            sys.executable,
            str(ROOT / "release_version.py"),
            version,
            "--github-output",
            str(github_output),
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert {
        "returncode": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "github_output": github_output.read_text(),
    } == {
        "returncode": 0,
        "stdout": "",
        "stderr": "",
        "github_output": f"python_version={python_version}\nrelease_tag={release_tag}\n",
    }


def test_stage_runtime_release_replaces_existing_staging_dir(tmp_path: Path) -> None:
    script = _load_update_script_module()
    staging_dir = tmp_path / "runtime-stage"
    old_file = staging_dir / "stale.txt"
    old_file.parent.mkdir(parents=True)
    old_file.write_text("stale")
    package_archive = _write_fake_codex_package_archive(tmp_path, script)

    staged = script.stage_python_runtime_package(
        staging_dir,
        "1.2.3",
        package_archive,
    )

    assert staged == staging_dir
    assert not old_file.exists()
    package_root = script.staged_runtime_package_root(staged)
    assert (package_root / "bin" / script.runtime_binary_name()).read_text() == "fake codex\n"


def test_stage_runtime_release_can_pin_wheel_platform_tag(tmp_path: Path) -> None:
    script = _load_update_script_module()
    package_archive = _write_fake_codex_package_archive(tmp_path, script)

    staged = script.stage_python_runtime_package(
        tmp_path / "runtime-stage",
        "0.116.0a1",
        package_archive,
        platform_tag="manylinux_2_17_x86_64",
    )

    pyproject = (staged / "pyproject.toml").read_text()
    assert 'platform-tag = "manylinux_2_17_x86_64"' in pyproject


@pytest.mark.parametrize("source_name", ["codex-package.tar.gz", "codex-package"])
def test_stage_runtime_release_rejects_incomplete_package_layout(
    tmp_path: Path, source_name: str
) -> None:
    script = _load_update_script_module()
    package_dir = tmp_path / "codex-package"
    (package_dir / "bin").mkdir(parents=True)
    package_archive = tmp_path / "codex-package.tar.gz"
    _write_package_archive(package_dir, package_archive)

    with pytest.raises(RuntimeError, match="Missing Codex package layout entries"):
        script.stage_python_runtime_package(
            tmp_path / "runtime-stage", "1.2.3", tmp_path / source_name
        )


def test_stage_runtime_directory_matches_archive(tmp_path: Path) -> None:
    script = _load_update_script_module()
    package_dir = _write_fake_codex_package(tmp_path / "codex-package", script)
    (package_dir / "bin" / script.runtime_binary_name()).chmod(0o755)
    package_archive = tmp_path / "codex-package.tar.gz"
    _write_package_archive(package_dir, package_archive)

    staged_trees = []
    for name, source in [("archive", package_archive), ("directory", package_dir)]:
        staged = script.stage_python_runtime_package(
            tmp_path / name, "1.2.3", source, platform_tag="win_amd64"
        )
        staged_trees.append(
            {
                path.relative_to(staged): (path.read_bytes(), path.stat().st_mode & 0o777)
                for path in staged.rglob("*")
                if path.is_file()
            }
        )

    assert staged_trees[0] == staged_trees[1]
    (script.staged_runtime_package_root(tmp_path / "directory") / "codex-package.json").unlink()
    assert (package_dir / "codex-package.json").is_file()


@pytest.mark.parametrize("entry_kind", ["file-link", "directory-link", "fifo"])
def test_stage_runtime_directory_rejects_non_regular_entries(
    tmp_path: Path, entry_kind: str
) -> None:
    script = _load_update_script_module()
    package_dir = _write_fake_codex_package(tmp_path / "codex-package", script)
    entry = package_dir / "codex-resources" / "invalid"
    if entry_kind == "fifo":
        if not hasattr(os, "mkfifo"):
            pytest.skip("FIFOs are not available on this platform")
        os.mkfifo(entry)
    else:
        target = package_dir / ("codex-package.json" if entry_kind == "file-link" else "bin")
        try:
            entry.symlink_to(target, target_is_directory=target.is_dir())
        except OSError:
            pytest.skip("Symlinks are not available on this platform")

    with pytest.raises(RuntimeError, match="Expected a regular Codex package entry"):
        script.stage_python_runtime_package(tmp_path / "runtime-stage", "1.2.3", package_dir)


@pytest.mark.parametrize("staging_path", ["codex-package", "codex-package/stage", "."])
def test_stage_runtime_directory_rejects_overlapping_staging(
    tmp_path: Path, staging_path: str
) -> None:
    script = _load_update_script_module()
    package_dir = _write_fake_codex_package(tmp_path / "codex-package", script)

    with pytest.raises(RuntimeError, match="directories must not overlap"):
        script.stage_python_runtime_package(tmp_path / staging_path, "1.2.3", package_dir)

    assert (package_dir / "codex-package.json").is_file()


def test_runtime_package_layout_is_included_by_wheel_config(
    tmp_path: Path,
) -> None:
    script = _load_update_script_module()
    package_archive = _write_fake_codex_package_archive(tmp_path, script)

    staged = script.stage_python_runtime_package(
        tmp_path / "runtime-stage",
        "1.2.3",
        package_archive,
    )

    pyproject = tomllib.loads((staged / "pyproject.toml").read_text())
    assert pyproject["tool"]["hatch"]["build"]["targets"]["wheel"]["include"] == [
        "src/codex_cli_bin/codex-package.json",
        "src/codex_cli_bin/bin/**",
        "src/codex_cli_bin/codex-resources/**",
        "src/codex_cli_bin/codex-path/**",
    ]


@pytest.fixture
def sdk_release_source(tmp_path: Path) -> Path:
    script = _load_update_script_module()
    source = tmp_path / "sdk-source"
    script._copy_package_tree(ROOT, source)
    project = source / "pyproject.toml"
    project.write_text(
        re.sub(
            r"openai-codex-cli-bin==[^\"\s]+", "openai-codex-cli-bin==0.153.0", project.read_text()
        )
    )
    return source


def test_stage_sdk_release_packages_reviewed_artifacts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, sdk_release_source: Path
) -> None:
    script = _load_update_script_module()
    monkeypatch.setattr(script, "sdk_root", lambda: sdk_release_source)
    staged = tmp_path / "sdk-stage"
    source_project = tomllib.loads((sdk_release_source / "pyproject.toml").read_text())
    generated_paths = [
        "src/openai_codex/generated/v2_all.py",
        "src/openai_codex/generated/notification_registry.py",
        "src/openai_codex/api.py",
    ]
    reviewed_artifacts = {path: (ROOT / path).read_bytes() for path in generated_paths}

    script.main(
        [
            "stage-sdk",
            str(staged),
            "--sdk-version",
            "0.153.0",
        ]
    )

    pyproject = tomllib.loads((staged / "pyproject.toml").read_text())
    assert {
        "name": pyproject["project"]["name"],
        "version": pyproject["project"]["version"],
        "dependencies": pyproject["project"]["dependencies"],
    } == {
        "name": "openai-codex",
        "version": "0.153.0",
        "dependencies": source_project["project"]["dependencies"],
    }
    assert {path: (staged / path).read_bytes() for path in generated_paths} == reviewed_artifacts
    assert (
        '__version__ = "0.147.0"'
        not in (staged / "src" / "openai_codex" / "__init__.py").read_text()
    )
    assert (
        'client_version: str = "0.147.0"'
        not in (staged / "src" / "openai_codex" / "client.py").read_text()
    )
    assert not any((staged / "src" / "openai_codex").glob("bin/**"))


@pytest.mark.parametrize("source_runtime", ["0.147.0", "0.153.0"])
@pytest.mark.parametrize("sdk_version", ["0.154.0", "0.2.0b1"])
def test_built_sdk_uses_explicit_release_versions(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    sdk_release_source: Path,
    sdk_version: str,
    source_runtime: str,
) -> None:
    script = _load_update_script_module()
    monkeypatch.setattr(script, "sdk_root", lambda: sdk_release_source)
    project_path = sdk_release_source / "pyproject.toml"
    project_path.write_text(project_path.read_text().replace("==0.153.0", f"=={source_runtime}"))
    source_project = project_path.read_bytes()
    expected_dependencies = {
        *tomllib.loads(source_project.decode())["project"]["dependencies"],
        "openai-codex-cli-bin==0.154.0",
    } - {f"openai-codex-cli-bin=={source_runtime}"}
    reviewed_files = {
        path: (sdk_release_source / path).read_bytes()
        for path in (
            "src/openai_codex/generated/v2_all.py",
            "src/openai_codex/generated/notification_registry.py",
            "src/openai_codex/api.py",
        )
    }
    staged = script.stage_python_sdk_package(tmp_path / "sdk-stage", sdk_version, "rust-v0.154.0")
    dist = tmp_path / "dist"
    subprocess.run(
        ["uv", "build", "--wheel", "--sdist", "--out-dir", str(dist), str(staged)],
        check=True,
        capture_output=True,
        text=True,
    )

    with zipfile.ZipFile(next(dist.glob("*.whl"))) as wheel:
        metadata = [
            wheel.read(next(name for name in wheel.namelist() if name.endswith("/METADATA")))
        ]
        assert {
            path: wheel.read(path.removeprefix("src/")) for path in reviewed_files
        } == reviewed_files
    with tarfile.open(next(dist.glob("*.tar.gz"))) as sdist:
        prefix = f"openai_codex-{sdk_version}/"
        metadata.append(sdist.extractfile(prefix + "PKG-INFO").read())
        assert {
            path: sdist.extractfile(prefix + path).read() for path in reviewed_files
        } == reviewed_files
    for content in metadata:
        package = BytesParser().parsebytes(content)
        assert {
            "name": package["Name"],
            "version": package["Version"],
            "dependencies": set(package.get_all("Requires-Dist")),
        } == {
            "name": "openai-codex",
            "version": sdk_version,
            "dependencies": expected_dependencies,
        }
    assert (sdk_release_source / "pyproject.toml").read_bytes() == source_project
    assert {
        path: (sdk_release_source / path).read_bytes() for path in reviewed_files
    } == reviewed_files


def test_stage_sdk_release_replaces_existing_staging_dir(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, sdk_release_source: Path
) -> None:
    script = _load_update_script_module()
    monkeypatch.setattr(script, "sdk_root", lambda: sdk_release_source)
    staging_dir = tmp_path / "sdk-stage"
    old_file = staging_dir / "stale.txt"
    old_file.parent.mkdir(parents=True)
    old_file.write_text("stale")

    staged = script.stage_python_sdk_package(staging_dir, "0.153.0")

    assert staged == staging_dir
    assert not old_file.exists()


def test_sdk_release_matches_stable_runtime(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, sdk_release_source: Path
) -> None:
    script = _load_update_script_module()
    monkeypatch.setattr(script, "sdk_root", lambda: sdk_release_source)
    package_archive = _write_fake_codex_package_archive(tmp_path, script)

    sdk_stage = script.stage_python_sdk_package(
        tmp_path / "sdk-stage",
        "0.153.0",
    )
    runtime_stage = script.stage_python_runtime_package(
        tmp_path / "runtime-stage",
        "0.153.0",
        package_archive,
    )

    sdk_pyproject = tomllib.loads((sdk_stage / "pyproject.toml").read_text())
    runtime_pyproject = tomllib.loads((runtime_stage / "pyproject.toml").read_text())

    assert {
        "sdk_version": sdk_pyproject["project"]["version"],
        "runtime_version": runtime_pyproject["project"]["version"],
        "sdk_dependencies": sdk_pyproject["project"]["dependencies"],
    } == {
        "sdk_version": "0.153.0",
        "runtime_version": "0.153.0",
        "sdk_dependencies": [
            "pydantic>=2.12",
            "packaging>=26.2",
            "openai-codex-cli-bin==0.153.0",
        ],
    }


@pytest.mark.parametrize("runtime_version", ["0.149.0", "0.151.0a1", "0.0.0", "unknown"])
def test_sdk_release_rejects_unsupported_runtime_even_for_beta(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    sdk_release_source: Path,
    runtime_version: str,
) -> None:
    script = _load_update_script_module()
    monkeypatch.setattr(script, "sdk_root", lambda: sdk_release_source)
    project = sdk_release_source / "pyproject.toml"
    project.write_text(project.read_text().replace("==0.153.0", f"=={runtime_version}"))

    with pytest.raises(RuntimeError, match=r"Cannot package.*Codex CLI 0\.151\.0 or newer"):
        script.stage_python_sdk_package(tmp_path / "sdk-stage", "0.1.0b1")


def test_sdk_runtime_override_is_checked_after_stamping(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, sdk_release_source: Path
) -> None:
    script = _load_update_script_module()
    monkeypatch.setattr(script, "sdk_root", lambda: sdk_release_source)
    with pytest.raises(RuntimeError, match=r"Cannot package.*Codex CLI 0\.151\.0 or newer"):
        script.stage_python_sdk_package(tmp_path / "sdk-stage", "0.1.0b1", "0.149.0")


def test_sdk_beta_can_use_a_supported_runtime(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, sdk_release_source: Path
) -> None:
    script = _load_update_script_module()
    monkeypatch.setattr(script, "sdk_root", lambda: sdk_release_source)

    staged = script.stage_python_sdk_package(tmp_path / "sdk-stage", "0.1.0b1")

    project = tomllib.loads((staged / "pyproject.toml").read_text())["project"]
    assert (project["version"], project["dependencies"]) == (
        "0.1.0b1",
        ["pydantic>=2.12", "packaging>=26.2", "openai-codex-cli-bin==0.153.0"],
    )


@pytest.mark.parametrize("source_name", ["codex-package.tar.gz", "codex-package"])
def test_stage_runtime_stages_package_without_type_generation(
    tmp_path: Path, source_name: str
) -> None:
    script = _load_update_script_module()
    _write_fake_codex_package_archive(tmp_path, script)
    calls: list[str] = []
    args = script.parse_args(
        [
            "stage-runtime",
            str(tmp_path / "runtime-stage"),
            str(tmp_path / source_name),
            "--codex-version",
            "rust-v0.116.0-alpha.1",
            "--platform-tag",
            "manylinux_2_17_x86_64",
        ]
    )

    def fake_generate_types(_schema_dir: Path) -> None:
        calls.append("generate_types")

    def fake_stage_sdk_package(
        _staging_dir: Path, _sdk_version: str, _codex_version: str | None
    ) -> Path:
        raise AssertionError("sdk staging should not run for stage-runtime")

    def fake_stage_runtime_package(
        _staging_dir: Path,
        codex_version: str,
        package_archive: Path,
        platform_tag: str | None,
    ) -> Path:
        calls.append(f"stage_runtime:{codex_version}:{platform_tag}:{package_archive.name}")
        return tmp_path / "runtime-stage"

    ops = script.CliOps(
        generate_types=fake_generate_types,
        stage_python_sdk_package=fake_stage_sdk_package,
        stage_python_runtime_package=fake_stage_runtime_package,
    )

    script.run_command(args, ops)

    assert calls == [f"stage_runtime:0.116.0a1:manylinux_2_17_x86_64:{source_name}"]


def test_default_runtime_is_resolved_from_installed_runtime_package(
    tmp_path: Path,
) -> None:
    from openai_codex import client as client_module

    fake_binary = tmp_path / ("codex.exe" if client_module.os.name == "nt" else "codex")
    fake_binary.write_text("")
    ops = client_module.CodexBinResolverOps(
        installed_codex_path=lambda: fake_binary,
        path_exists=lambda path: path == fake_binary,
    )

    config = client_module.CodexConfig()
    assert config.codex_bin is None
    assert client_module.resolve_codex_bin(config, ops) == fake_binary


def test_runtime_path_dir_is_prepended_without_duplicates(tmp_path: Path) -> None:
    from openai_codex import client as client_module

    path_dir = tmp_path / "codex-path"
    env = {"PATH": os.pathsep.join(["/usr/bin", str(path_dir), "/bin"])}

    client_module._prepend_path_dirs(env, (path_dir,))

    assert env["PATH"] == os.pathsep.join([str(path_dir), "/usr/bin", "/bin"])


def test_runtime_path_dir_preserves_windows_path_key(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    from openai_codex import client as client_module

    path_dir = tmp_path / "codex-path"
    monkeypatch.setattr(client_module.os, "name", "nt")
    env = {
        "PATH": "/usr/bin",
        "Path": os.pathsep.join(["C\\Windows", str(path_dir)]),
    }

    client_module._prepend_path_dirs(env, (path_dir,))

    assert env == {"Path": os.pathsep.join([str(path_dir), "C\\Windows"])}


def test_explicit_codex_bin_override_takes_priority(tmp_path: Path) -> None:
    from openai_codex import client as client_module

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

    config = client_module.CodexConfig(codex_bin=str(explicit_binary))
    assert client_module.resolve_codex_bin(config, ops) == explicit_binary


def test_missing_runtime_package_requires_explicit_codex_bin() -> None:
    from openai_codex import client as client_module

    ops = client_module.CodexBinResolverOps(
        installed_codex_path=lambda: (_ for _ in ()).throw(
            FileNotFoundError("missing packaged runtime")
        ),
        path_exists=lambda _path: False,
    )

    with pytest.raises(FileNotFoundError, match="missing packaged runtime"):
        client_module.resolve_codex_bin(client_module.CodexConfig(), ops)


def test_broken_runtime_package_does_not_fall_back() -> None:
    from openai_codex import client as client_module

    ops = client_module.CodexBinResolverOps(
        installed_codex_path=lambda: (_ for _ in ()).throw(
            FileNotFoundError("missing packaged binary")
        ),
        path_exists=lambda _path: False,
    )

    with pytest.raises(FileNotFoundError) as exc_info:
        client_module.resolve_codex_bin(client_module.CodexConfig(), ops)

    assert str(exc_info.value) == ("missing packaged binary")


@pytest.mark.parametrize("version", ["rust-v1.2.3-alpha", "rust-v1.2.3-beta.1", "invalid"])
def test_release_version_cli_rejects_unsupported_runtime_releases(
    tmp_path: Path, version: str
) -> None:
    github_output = tmp_path / "github-output"
    result = subprocess.run(
        [
            sys.executable,
            str(ROOT / "release_version.py"),
            version,
            "--github-output",
            str(github_output),
        ],
        text=True,
        capture_output=True,
        check=False,
    )
    assert result.returncode == 1
    assert not github_output.exists()


@pytest.mark.parametrize("runtime_dependency", ["", ', "openai-codex-cli-bin==1.2.3"' * 2])
def test_stage_sdk_release_rejects_missing_or_duplicate_runtime_pin(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    runtime_dependency: str,
) -> None:
    script = _load_update_script_module()
    template = tmp_path / "template"
    template.mkdir()
    (template / "pyproject.toml").write_text(
        '[project]\nname = "openai-codex"\nversion = "0.0.0"\n'
        f'dependencies = ["pydantic>=2.12"{runtime_dependency}]\n'
    )
    monkeypatch.setattr(script, "sdk_root", lambda: template)
    with pytest.raises(RuntimeError, match="Expected exactly one openai-codex-cli-bin"):
        script.stage_python_sdk_package(tmp_path / "sdk-stage", "1.2.3", "1.2.3")


def test_stage_sdk_rejects_empty_runtime_version(tmp_path: Path) -> None:
    script = _load_update_script_module()
    args = script.parse_args(
        ["stage-sdk", str(tmp_path / "sdk-stage"), "--sdk-version", "1.2.3", "--codex-version", ""]
    )
    with pytest.raises(RuntimeError, match="Could not normalize Codex version"):
        script.run_command(args, script.default_cli_ops())


def test_sdk_beta_can_pin_an_independent_runtime(tmp_path: Path) -> None:
    script = _load_update_script_module()
    staged = script.stage_python_sdk_package(tmp_path / "sdk-beta", "0.1.0b1", "0.153.0")
    project = tomllib.loads((staged / "pyproject.toml").read_text())["project"]
    assert (project["version"], project["dependencies"]) == (
        "0.1.0b1",
        ["pydantic>=2.12", "packaging>=26.2", "openai-codex-cli-bin==0.153.0"],
    )


@pytest.mark.parametrize(
    ("release_tag", "package_version"),
    [
        ("rust-v1.2.3", "1.2.3"),
        ("rust-v1.2.3-alpha.4", "1.2.3a4"),
        ("rust-v1.2.3-alpha.4.5", "1.2.3a4.post5"),
    ],
)
def test_sdk_release_matches_runtime(
    tmp_path: Path, release_tag: str, package_version: str
) -> None:
    script = _load_update_script_module()
    package_archive = _write_fake_codex_package_archive(tmp_path, script)
    source_pyproject = (script.sdk_root() / "pyproject.toml").read_text()

    sdk_stage = script.stage_python_sdk_package(
        tmp_path / "sdk-stage",
        release_tag,
        release_tag,
    )
    runtime_stage = script.stage_python_runtime_package(
        tmp_path / "runtime-stage",
        release_tag,
        package_archive,
    )

    sdk_pyproject = tomllib.loads((sdk_stage / "pyproject.toml").read_text())
    runtime_pyproject = tomllib.loads((runtime_stage / "pyproject.toml").read_text())

    assert {
        "sdk_version": sdk_pyproject["project"]["version"],
        "runtime_version": runtime_pyproject["project"]["version"],
        "sdk_dependencies": sdk_pyproject["project"]["dependencies"],
    } == {
        "sdk_version": package_version,
        "runtime_version": package_version,
        "sdk_dependencies": [
            "pydantic>=2.12",
            "packaging>=26.2",
            f"openai-codex-cli-bin=={package_version}",
        ],
    }
    assert (script.sdk_root() / "pyproject.toml").read_text() == source_pyproject
