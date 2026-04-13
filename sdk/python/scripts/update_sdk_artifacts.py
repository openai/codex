#!/usr/bin/env python3
from __future__ import annotations

import argparse
import importlib
import importlib.util
import json
import platform
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import types
import typing
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Sequence, get_args, get_origin

SDK_PKG_NAME = "openai-codex"
RUNTIME_PKG_NAME = "openai-codex-cli-bin"


def repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def sdk_root() -> Path:
    return repo_root() / "sdk" / "python"


def python_runtime_root() -> Path:
    return repo_root() / "sdk" / "python-runtime"


def schema_bundle_path(schema_dir: Path | None = None) -> Path:
    return schema_root_dir(schema_dir) / "codex_app_server_protocol.v2.schemas.json"


def schema_root_dir(schema_dir: Path | None = None) -> Path:
    if schema_dir is not None:
        return schema_dir
    return repo_root() / "codex-rs" / "app-server-protocol" / "schema" / "json"


def runtime_setup_path() -> Path:
    return sdk_root() / "_runtime_setup.py"


def _is_windows(system_name: str | None = None) -> bool:
    return (system_name or platform.system()).lower().startswith("win")


def runtime_binary_name(system_name: str | None = None) -> str:
    return "codex.exe" if _is_windows(system_name) else "codex"


def runtime_file_names(system_name: str | None = None) -> tuple[str, ...]:
    if _is_windows(system_name):
        return (
            "codex.exe",
            "codex-command-runner.exe",
            "codex-windows-sandbox-setup.exe",
        )
    return ("codex",)


def staged_runtime_bin_dir(root: Path) -> Path:
    return root / "src" / "codex_cli_bin" / "bin"


def staged_runtime_bin_path(root: Path) -> Path:
    return staged_runtime_bin_dir(root) / runtime_binary_name()


def run(cmd: list[str], cwd: Path) -> None:
    subprocess.run(cmd, cwd=str(cwd), check=True)


def run_python_module(module: str, args: list[str], cwd: Path) -> None:
    run([sys.executable, "-m", module, *args], cwd)


def current_sdk_version() -> str:
    match = re.search(
        r'^version = "([^"]+)"$',
        (sdk_root() / "pyproject.toml").read_text(),
        flags=re.MULTILINE,
    )
    if match is None:
        raise RuntimeError("Could not determine Python SDK version from pyproject.toml")
    return match.group(1)


def _copy_package_tree(src: Path, dst: Path) -> None:
    if dst.exists():
        if dst.is_dir():
            shutil.rmtree(dst)
        else:
            dst.unlink()
    shutil.copytree(
        src,
        dst,
        ignore=shutil.ignore_patterns(
            ".venv",
            ".venv2",
            ".pytest_cache",
            "__pycache__",
            "build",
            "dist",
            "*.pyc",
        ),
    )


def _rewrite_project_version(pyproject_text: str, version: str) -> str:
    updated, count = re.subn(
        r'^version = "[^"]+"$',
        f'version = "{version}"',
        pyproject_text,
        count=1,
        flags=re.MULTILINE,
    )
    if count != 1:
        raise RuntimeError("Could not rewrite project version in pyproject.toml")
    return updated


def _rewrite_project_name(pyproject_text: str, name: str) -> str:
    updated, count = re.subn(
        r'^name = "[^"]+"$',
        f'name = "{name}"',
        pyproject_text,
        count=1,
        flags=re.MULTILINE,
    )
    if count != 1:
        raise RuntimeError("Could not rewrite project name in pyproject.toml")
    return updated


def normalize_python_package_version(version: str) -> str:
    stripped = version.strip()
    if re.fullmatch(r"\d+\.\d+\.\d+(?:a\d+|b\d+|\.dev\d+)?", stripped):
        return stripped

    prerelease_match = re.fullmatch(
        r"(\d+\.\d+\.\d+)-(alpha|beta)\.(\d+)",
        stripped,
    )
    if prerelease_match is not None:
        base, prerelease, number = prerelease_match.groups()
        marker = "a" if prerelease == "alpha" else "b"
        return f"{base}{marker}{number}"

    raise RuntimeError(
        "Unsupported Python package version. Expected x.y.z, x.y.z-alpha.n, "
        f"x.y.z-beta.n, or an already-normalized PEP 440 version; got {version!r}."
    )


def _rewrite_sdk_runtime_dependency(pyproject_text: str, runtime_version: str) -> str:
    match = re.search(r"^dependencies = \[(.*?)\]$", pyproject_text, flags=re.MULTILINE)
    if match is None:
        raise RuntimeError(
            "Could not find dependencies array in sdk/python/pyproject.toml"
        )

    raw_items = [item.strip() for item in match.group(1).split(",") if item.strip()]
    raw_items = [
        item
        for item in raw_items
        if "codex-cli-bin" not in item and RUNTIME_PKG_NAME not in item
    ]
    raw_items.append(f'"{RUNTIME_PKG_NAME}=={runtime_version}"')
    replacement = "dependencies = [\n  " + ",\n  ".join(raw_items) + ",\n]"
    return pyproject_text[: match.start()] + replacement + pyproject_text[match.end() :]


def _rewrite_sdk_init_version(init_text: str, sdk_version: str) -> str:
    updated, count = re.subn(
        r'^__version__ = "[^"]+"$',
        f'__version__ = "{sdk_version}"',
        init_text,
        count=1,
        flags=re.MULTILINE,
    )
    if count != 1:
        raise RuntimeError("Could not rewrite SDK __version__")
    return updated


def _rewrite_sdk_client_version(client_text: str, sdk_version: str) -> str:
    updated, count = re.subn(
        r'client_version: str = "[^"]+"',
        f'client_version: str = "{sdk_version}"',
        client_text,
        count=1,
    )
    if count != 1:
        raise RuntimeError("Could not rewrite AppServerConfig.client_version")
    return updated


def stage_python_sdk_package(
    staging_dir: Path, sdk_version: str, runtime_version: str
) -> Path:
    sdk_version = normalize_python_package_version(sdk_version)
    runtime_version = normalize_python_package_version(runtime_version)
    _copy_package_tree(sdk_root(), staging_dir)
    sdk_bin_dir = staging_dir / "src" / "codex_app_server" / "bin"
    if sdk_bin_dir.exists():
        shutil.rmtree(sdk_bin_dir)

    pyproject_path = staging_dir / "pyproject.toml"
    pyproject_text = pyproject_path.read_text()
    pyproject_text = _rewrite_project_name(pyproject_text, SDK_PKG_NAME)
    pyproject_text = _rewrite_project_version(pyproject_text, sdk_version)
    pyproject_text = _rewrite_sdk_runtime_dependency(pyproject_text, runtime_version)
    pyproject_path.write_text(pyproject_text)

    init_path = staging_dir / "src" / "codex_app_server" / "__init__.py"
    init_path.write_text(_rewrite_sdk_init_version(init_path.read_text(), sdk_version))

    client_path = staging_dir / "src" / "codex_app_server" / "client.py"
    client_path.write_text(
        _rewrite_sdk_client_version(client_path.read_text(), sdk_version)
    )
    return staging_dir


def stage_python_runtime_package(
    staging_dir: Path, runtime_version: str, runtime_bundle_dir: Path
) -> Path:
    runtime_version = normalize_python_package_version(runtime_version)
    _copy_package_tree(python_runtime_root(), staging_dir)

    pyproject_path = staging_dir / "pyproject.toml"
    pyproject_text = _rewrite_project_name(pyproject_path.read_text(), RUNTIME_PKG_NAME)
    pyproject_text = _rewrite_project_version(pyproject_text, runtime_version)
    pyproject_path.write_text(pyproject_text)

    out_bin_dir = staged_runtime_bin_dir(staging_dir)
    out_bin_dir.mkdir(parents=True, exist_ok=True)
    for runtime_file_name in runtime_file_names():
        source = _find_runtime_bundle_file(runtime_bundle_dir, runtime_file_name)
        out_path = out_bin_dir / runtime_file_name
        shutil.copy2(source, out_path)
        if not _is_windows():
            out_path.chmod(
                out_path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH
            )
    return staging_dir


def _find_runtime_bundle_file(runtime_bundle_dir: Path, destination_name: str) -> Path:
    if not runtime_bundle_dir.is_dir():
        raise RuntimeError(f"Runtime bundle directory not found: {runtime_bundle_dir}")

    exact = runtime_bundle_dir / destination_name
    if exact.is_file():
        return exact

    patterns = {
        "codex": re.compile(r"^codex-(?!responses-api-proxy)[^.]+$"),
        "codex.exe": re.compile(
            r"^codex-(?!command-runner|windows-sandbox-setup|responses-api-proxy).+\.exe$"
        ),
        "codex-command-runner.exe": re.compile(r"^codex-command-runner-.+\.exe$"),
        "codex-windows-sandbox-setup.exe": re.compile(
            r"^codex-windows-sandbox-setup-.+\.exe$"
        ),
    }
    pattern = patterns.get(destination_name)
    candidates = (
        []
        if pattern is None
        else sorted(
            path
            for path in runtime_bundle_dir.iterdir()
            if path.is_file() and pattern.fullmatch(path.name)
        )
    )
    if len(candidates) == 1:
        return candidates[0]
    if len(candidates) > 1:
        candidate_names = ", ".join(path.name for path in candidates)
        raise RuntimeError(
            f"Runtime bundle has multiple candidates for {destination_name}: "
            f"{candidate_names}"
        )

    raise RuntimeError(
        f"Runtime bundle {runtime_bundle_dir} is missing required file "
        f"{destination_name}"
    )


def _load_runtime_setup_module() -> Any:
    spec = importlib.util.spec_from_file_location(
        "_codex_python_runtime_setup", runtime_setup_path()
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Failed to load {runtime_setup_path()}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _bundled_codex_path_from_install_target(install_target: Path) -> Path:
    package_init = install_target / "codex_cli_bin" / "__init__.py"
    spec = importlib.util.spec_from_file_location(
        "_codex_cli_bin_for_schema",
        package_init,
        submodule_search_locations=[str(package_init.parent)],
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Failed to load installed runtime package: {package_init}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module.bundled_codex_path()


def _run_runtime_schema_generator(codex_bin: Path, out_dir: Path) -> None:
    run(
        [
            str(codex_bin),
            "app-server",
            "generate-json-schema",
            "--out",
            str(out_dir),
        ],
        cwd=repo_root(),
    )


def _generate_json_schema_from_runtime(
    out_dir: Path, runtime_version: str | None = None
) -> str:
    runtime_setup = _load_runtime_setup_module()
    requested_version = runtime_version or runtime_setup.pinned_runtime_version()
    with tempfile.TemporaryDirectory(prefix="codex-python-schema-runtime-") as td:
        install_target = Path(td) / "runtime-package"
        original_pinned_runtime_version = runtime_setup.PINNED_RUNTIME_VERSION
        runtime_setup.PINNED_RUNTIME_VERSION = requested_version
        try:
            runtime_setup.ensure_runtime_package_installed(
                sys.executable,
                sdk_root(),
                install_target,
            )
        finally:
            runtime_setup.PINNED_RUNTIME_VERSION = original_pinned_runtime_version
        codex_bin = _bundled_codex_path_from_install_target(install_target)
        _run_runtime_schema_generator(codex_bin, out_dir)
    return requested_version


def _flatten_string_enum_one_of(definition: dict[str, Any]) -> bool:
    branches = definition.get("oneOf")
    if not isinstance(branches, list) or not branches:
        return False

    enum_values: list[str] = []
    for branch in branches:
        if not isinstance(branch, dict):
            return False
        if branch.get("type") != "string":
            return False

        enum = branch.get("enum")
        if not isinstance(enum, list) or len(enum) != 1 or not isinstance(enum[0], str):
            return False

        extra_keys = set(branch) - {"type", "enum", "description", "title"}
        if extra_keys:
            return False

        enum_values.append(enum[0])

    description = definition.get("description")
    title = definition.get("title")
    definition.clear()
    definition["type"] = "string"
    definition["enum"] = enum_values
    if isinstance(description, str):
        definition["description"] = description
    if isinstance(title, str):
        definition["title"] = title
    return True


DISCRIMINATOR_KEYS = ("type", "method", "mode", "state", "status", "role", "reason")


def _to_pascal_case(value: str) -> str:
    parts = re.split(r"[^0-9A-Za-z]+", value)
    compact = "".join(part[:1].upper() + part[1:] for part in parts if part)
    return compact or "Value"


def _string_literal(value: Any) -> str | None:
    if not isinstance(value, dict):
        return None
    const = value.get("const")
    if isinstance(const, str):
        return const

    enum = value.get("enum")
    if isinstance(enum, list) and enum and len(enum) == 1 and isinstance(enum[0], str):
        return enum[0]
    return None


def _enum_literals(value: Any) -> list[str] | None:
    if not isinstance(value, dict):
        return None
    enum = value.get("enum")
    if (
        not isinstance(enum, list)
        or not enum
        or not all(isinstance(item, str) for item in enum)
    ):
        return None
    return list(enum)


def _literal_from_property(props: dict[str, Any], key: str) -> str | None:
    return _string_literal(props.get(key))


def _variant_definition_name(base: str, variant: dict[str, Any]) -> str | None:
    # datamodel-code-generator invents numbered helper names for inline union
    # branches unless they carry a stable, unique title up front. We derive
    # those titles from the branch discriminator or other identifying shape.
    props = variant.get("properties")
    if isinstance(props, dict):
        for key in DISCRIMINATOR_KEYS:
            literal = _literal_from_property(props, key)
            if literal is None:
                continue
            pascal = _to_pascal_case(literal)
            if base == "ClientRequest":
                return f"{pascal}Request"
            if base == "ServerRequest":
                return f"{pascal}ServerRequest"
            if base == "ClientNotification":
                return f"{pascal}ClientNotification"
            if base == "ServerNotification":
                return f"{pascal}ServerNotification"
            if base == "EventMsg":
                return f"{pascal}EventMsg"
            return f"{pascal}{base}"

        if len(props) == 1:
            key = next(iter(props))
            pascal = _string_literal(props[key])
            return f"{_to_pascal_case(pascal or key)}{base}"

    required = variant.get("required")
    if (
        isinstance(required, list)
        and len(required) == 1
        and isinstance(required[0], str)
    ):
        return f"{_to_pascal_case(required[0])}{base}"

    enum_literals = _enum_literals(variant)
    if enum_literals is not None:
        if len(enum_literals) == 1:
            return f"{_to_pascal_case(enum_literals[0])}{base}"
        return f"{base}Value"

    return None


def _variant_collision_key(
    base: str, variant: dict[str, Any], generated_name: str
) -> str:
    parts = [f"base={base}", f"generated={generated_name}"]
    props = variant.get("properties")
    if isinstance(props, dict):
        for key in DISCRIMINATOR_KEYS:
            literal = _literal_from_property(props, key)
            if literal is not None:
                parts.append(f"{key}={literal}")
        if len(props) == 1:
            parts.append(f"only_property={next(iter(props))}")

    required = variant.get("required")
    if (
        isinstance(required, list)
        and len(required) == 1
        and isinstance(required[0], str)
    ):
        parts.append(f"required_only={required[0]}")

    enum_literals = _enum_literals(variant)
    if enum_literals is not None:
        parts.append(f"enum={'|'.join(enum_literals)}")

    return "|".join(parts)


def _set_discriminator_titles(props: dict[str, Any], owner: str) -> None:
    for key in DISCRIMINATOR_KEYS:
        prop = props.get(key)
        if not isinstance(prop, dict):
            continue
        if _string_literal(prop) is None or "title" in prop:
            continue
        prop["title"] = f"{owner}{_to_pascal_case(key)}"


def _annotate_variant_list(variants: list[Any], base: str | None) -> None:
    seen = {
        variant["title"]
        for variant in variants
        if isinstance(variant, dict) and isinstance(variant.get("title"), str)
    }

    for variant in variants:
        if not isinstance(variant, dict):
            continue

        variant_name = variant.get("title")
        generated_name = _variant_definition_name(base, variant) if base else None
        if generated_name is not None and (
            not isinstance(variant_name, str)
            or "/" in variant_name
            or variant_name != generated_name
        ):
            # Titles like `Thread/startedNotification` sanitize poorly in
            # Python, and envelope titles like `ErrorNotification` collide
            # with their payload model names. Rewrite them before codegen so
            # we get `ThreadStartedServerNotification` instead of `...1`.
            if generated_name in seen and variant_name != generated_name:
                raise RuntimeError(
                    "Variant title naming collision detected: "
                    f"{_variant_collision_key(base or '<root>', variant, generated_name)}"
                )
            variant["title"] = generated_name
            seen.add(generated_name)
            variant_name = generated_name

        if isinstance(variant_name, str):
            props = variant.get("properties")
            if isinstance(props, dict):
                _set_discriminator_titles(props, variant_name)

        _annotate_schema(variant, base)


def _annotate_schema(value: Any, base: str | None = None) -> None:
    if isinstance(value, list):
        for item in value:
            _annotate_schema(item, base)
        return

    if not isinstance(value, dict):
        return

    owner = value.get("title")
    props = value.get("properties")
    if isinstance(owner, str) and isinstance(props, dict):
        _set_discriminator_titles(props, owner)

    one_of = value.get("oneOf")
    if isinstance(one_of, list):
        # Walk nested unions recursively so every inline branch gets the same
        # title normalization treatment before we hand the bundle to Python
        # codegen.
        _annotate_variant_list(one_of, base)

    any_of = value.get("anyOf")
    if isinstance(any_of, list):
        _annotate_variant_list(any_of, base)

    definitions = value.get("definitions")
    if isinstance(definitions, dict):
        for name, schema in definitions.items():
            _annotate_schema(schema, name if isinstance(name, str) else base)

    defs = value.get("$defs")
    if isinstance(defs, dict):
        for name, schema in defs.items():
            _annotate_schema(schema, name if isinstance(name, str) else base)

    for key, child in value.items():
        if key in {"oneOf", "anyOf", "definitions", "$defs"}:
            continue
        _annotate_schema(child, base)


def _normalized_schema_bundle_text(schema_dir: Path | None = None) -> str:
    schema = json.loads(schema_bundle_path(schema_dir).read_text())
    definitions = schema.get("definitions", {})
    if isinstance(definitions, dict):
        for definition in definitions.values():
            if isinstance(definition, dict):
                _flatten_string_enum_one_of(definition)
    # Normalize the schema into something datamodel-code-generator can map to
    # stable class names instead of anonymous numbered helpers.
    _annotate_schema(schema)
    return json.dumps(schema, indent=2, sort_keys=True) + "\n"


def generate_v2_all(schema_dir: Path | None = None) -> None:
    out_path = sdk_root() / "src" / "codex_app_server" / "generated" / "v2_all.py"
    out_dir = out_path.parent
    old_package_dir = out_dir / "v2_all"
    if old_package_dir.exists():
        shutil.rmtree(old_package_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as td:
        normalized_bundle = Path(td) / schema_bundle_path(schema_dir).name
        normalized_bundle.write_text(_normalized_schema_bundle_text(schema_dir))
        run_python_module(
            "datamodel_code_generator",
            [
                "--input",
                str(normalized_bundle),
                "--input-file-type",
                "jsonschema",
                "--output",
                str(out_path),
                "--output-model-type",
                "pydantic_v2.BaseModel",
                "--target-python-version",
                "3.11",
                "--use-standard-collections",
                "--enum-field-as-literal",
                "one",
                "--field-constraints",
                "--use-default-kwarg",
                "--snake-case-field",
                "--allow-population-by-field-name",
                # Once the schema prepass has assigned stable titles, tell the
                # generator to prefer those titles as the emitted class names.
                "--use-title-as-name",
                "--use-annotated",
                "--use-union-operator",
                "--disable-timestamp",
                # Keep the generated file formatted deterministically so the
                # checked-in artifact only changes when the schema does.
                "--formatters",
                "ruff-format",
            ],
            cwd=sdk_root(),
        )
    _normalize_generated_timestamps(out_path)


def _notification_specs(schema_dir: Path | None = None) -> list[tuple[str, str]]:
    server_notifications = json.loads(
        (schema_root_dir(schema_dir) / "ServerNotification.json").read_text()
    )
    one_of = server_notifications.get("oneOf", [])
    generated_source = (
        sdk_root() / "src" / "codex_app_server" / "generated" / "v2_all.py"
    ).read_text()

    specs: list[tuple[str, str]] = []

    for variant in one_of:
        props = variant.get("properties", {})
        method_meta = props.get("method", {})
        params_meta = props.get("params", {})

        methods = method_meta.get("enum", [])
        if len(methods) != 1:
            continue
        method = methods[0]
        if not isinstance(method, str):
            continue

        ref = params_meta.get("$ref")
        if not isinstance(ref, str) or not ref.startswith("#/definitions/"):
            continue
        class_name = ref.split("/")[-1]
        if (
            f"class {class_name}(" not in generated_source
            and f"{class_name} =" not in generated_source
        ):
            # Skip schema variants that are not emitted into the generated v2 surface.
            continue
        specs.append((method, class_name))

    specs.sort()
    return specs


def generate_notification_registry(schema_dir: Path | None = None) -> None:
    out = (
        sdk_root()
        / "src"
        / "codex_app_server"
        / "generated"
        / "notification_registry.py"
    )
    specs = _notification_specs(schema_dir)
    class_names = sorted({class_name for _, class_name in specs})

    lines = [
        "# Auto-generated by scripts/update_sdk_artifacts.py",
        "# DO NOT EDIT MANUALLY.",
        "",
        "from __future__ import annotations",
        "",
        "from pydantic import BaseModel",
        "",
    ]

    for class_name in class_names:
        lines.append(f"from .v2_all import {class_name}")
    lines.extend(
        [
            "",
            "NOTIFICATION_MODELS: dict[str, type[BaseModel]] = {",
        ]
    )
    for method, class_name in specs:
        lines.append(f'    "{method}": {class_name},')
    lines.extend(["}", ""])

    out.write_text("\n".join(lines))


def _normalize_generated_timestamps(root: Path) -> None:
    timestamp_re = re.compile(r"^#\s+timestamp:\s+.+$", flags=re.MULTILINE)
    py_files = [root] if root.is_file() else sorted(root.rglob("*.py"))
    for py_file in py_files:
        content = py_file.read_text()
        normalized = timestamp_re.sub("#   timestamp: <normalized>", content)
        if normalized != content:
            py_file.write_text(normalized)


FIELD_ANNOTATION_OVERRIDES: dict[str, str] = {
    # Keep public API typed without falling back to `Any`.
    "config": "JsonObject",
    "output_schema": "JsonObject",
}


@dataclass(slots=True)
class PublicFieldSpec:
    wire_name: str
    py_name: str
    annotation: str
    required: bool


@dataclass(frozen=True)
class CliOps:
    generate_types: Callable[[str | None], None]
    stage_python_sdk_package: Callable[[Path, str, str], Path]
    stage_python_runtime_package: Callable[[Path, str, Path], Path]
    current_sdk_version: Callable[[], str]


def _annotation_to_source(annotation: Any) -> str:
    origin = get_origin(annotation)
    if origin is typing.Annotated:
        return _annotation_to_source(get_args(annotation)[0])
    if origin in (typing.Union, types.UnionType):
        parts: list[str] = []
        for arg in get_args(annotation):
            rendered = _annotation_to_source(arg)
            if rendered not in parts:
                parts.append(rendered)
        return " | ".join(parts)
    if origin is list:
        args = get_args(annotation)
        item = _annotation_to_source(args[0]) if args else "Any"
        return f"list[{item}]"
    if origin is dict:
        args = get_args(annotation)
        key = _annotation_to_source(args[0]) if args else "str"
        val = _annotation_to_source(args[1]) if len(args) > 1 else "Any"
        return f"dict[{key}, {val}]"
    if annotation is Any or annotation is typing.Any:
        return "Any"
    if annotation is None or annotation is type(None):
        return "None"
    if isinstance(annotation, type):
        if annotation.__module__ == "builtins":
            return annotation.__name__
        return annotation.__name__
    return repr(annotation)


def _camel_to_snake(name: str) -> str:
    head = re.sub(r"(.)([A-Z][a-z]+)", r"\1_\2", name)
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", head).lower()


def _load_public_fields(
    module_name: str, class_name: str, *, exclude: set[str] | None = None
) -> list[PublicFieldSpec]:
    exclude = exclude or set()
    module = importlib.import_module(module_name)
    model = getattr(module, class_name)
    fields: list[PublicFieldSpec] = []
    for name, field in model.model_fields.items():
        if name in exclude:
            continue
        required = field.is_required()
        annotation = _annotation_to_source(field.annotation)
        override = FIELD_ANNOTATION_OVERRIDES.get(name)
        if override is not None:
            annotation = override if required else f"{override} | None"
        fields.append(
            PublicFieldSpec(
                wire_name=name,
                py_name=name,
                annotation=annotation,
                required=required,
            )
        )
    return fields


def _kw_signature_lines(fields: list[PublicFieldSpec]) -> list[str]:
    lines: list[str] = []
    for field in fields:
        default = "" if field.required else " = None"
        lines.append(f"        {field.py_name}: {field.annotation}{default},")
    return lines


def _model_arg_lines(
    fields: list[PublicFieldSpec], *, indent: str = "            "
) -> list[str]:
    return [f"{indent}{field.wire_name}={field.py_name}," for field in fields]


def _replace_generated_block(source: str, block_name: str, body: str) -> str:
    start_tag = f"    # BEGIN GENERATED: {block_name}"
    end_tag = f"    # END GENERATED: {block_name}"
    pattern = re.compile(rf"(?s){re.escape(start_tag)}\n.*?\n{re.escape(end_tag)}")
    replacement = f"{start_tag}\n{body.rstrip()}\n{end_tag}"
    updated, count = pattern.subn(replacement, source, count=1)
    if count != 1:
        raise RuntimeError(f"Could not update generated block: {block_name}")
    return updated


def _render_codex_block(
    thread_start_fields: list[PublicFieldSpec],
    thread_list_fields: list[PublicFieldSpec],
    resume_fields: list[PublicFieldSpec],
    fork_fields: list[PublicFieldSpec],
) -> str:
    lines = [
        "    def thread_start(",
        "        self,",
        "        *,",
        *_kw_signature_lines(thread_start_fields),
        "    ) -> Thread:",
        "        params = ThreadStartParams(",
        *_model_arg_lines(thread_start_fields),
        "        )",
        "        started = self._client.thread_start(params)",
        "        return Thread(self._client, started.thread.id)",
        "",
        "    def thread_list(",
        "        self,",
        "        *,",
        *_kw_signature_lines(thread_list_fields),
        "    ) -> ThreadListResponse:",
        "        params = ThreadListParams(",
        *_model_arg_lines(thread_list_fields),
        "        )",
        "        return self._client.thread_list(params)",
        "",
        "    def thread_resume(",
        "        self,",
        "        thread_id: str,",
        "        *,",
        *_kw_signature_lines(resume_fields),
        "    ) -> Thread:",
        "        params = ThreadResumeParams(",
        "            thread_id=thread_id,",
        *_model_arg_lines(resume_fields),
        "        )",
        "        resumed = self._client.thread_resume(thread_id, params)",
        "        return Thread(self._client, resumed.thread.id)",
        "",
        "    def thread_fork(",
        "        self,",
        "        thread_id: str,",
        "        *,",
        *_kw_signature_lines(fork_fields),
        "    ) -> Thread:",
        "        params = ThreadForkParams(",
        "            thread_id=thread_id,",
        *_model_arg_lines(fork_fields),
        "        )",
        "        forked = self._client.thread_fork(thread_id, params)",
        "        return Thread(self._client, forked.thread.id)",
        "",
        "    def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:",
        "        return self._client.thread_archive(thread_id)",
        "",
        "    def thread_unarchive(self, thread_id: str) -> Thread:",
        "        unarchived = self._client.thread_unarchive(thread_id)",
        "        return Thread(self._client, unarchived.thread.id)",
    ]
    return "\n".join(lines)


def _render_async_codex_block(
    thread_start_fields: list[PublicFieldSpec],
    thread_list_fields: list[PublicFieldSpec],
    resume_fields: list[PublicFieldSpec],
    fork_fields: list[PublicFieldSpec],
) -> str:
    lines = [
        "    async def thread_start(",
        "        self,",
        "        *,",
        *_kw_signature_lines(thread_start_fields),
        "    ) -> AsyncThread:",
        "        await self._ensure_initialized()",
        "        params = ThreadStartParams(",
        *_model_arg_lines(thread_start_fields),
        "        )",
        "        started = await self._client.thread_start(params)",
        "        return AsyncThread(self, started.thread.id)",
        "",
        "    async def thread_list(",
        "        self,",
        "        *,",
        *_kw_signature_lines(thread_list_fields),
        "    ) -> ThreadListResponse:",
        "        await self._ensure_initialized()",
        "        params = ThreadListParams(",
        *_model_arg_lines(thread_list_fields),
        "        )",
        "        return await self._client.thread_list(params)",
        "",
        "    async def thread_resume(",
        "        self,",
        "        thread_id: str,",
        "        *,",
        *_kw_signature_lines(resume_fields),
        "    ) -> AsyncThread:",
        "        await self._ensure_initialized()",
        "        params = ThreadResumeParams(",
        "            thread_id=thread_id,",
        *_model_arg_lines(resume_fields),
        "        )",
        "        resumed = await self._client.thread_resume(thread_id, params)",
        "        return AsyncThread(self, resumed.thread.id)",
        "",
        "    async def thread_fork(",
        "        self,",
        "        thread_id: str,",
        "        *,",
        *_kw_signature_lines(fork_fields),
        "    ) -> AsyncThread:",
        "        await self._ensure_initialized()",
        "        params = ThreadForkParams(",
        "            thread_id=thread_id,",
        *_model_arg_lines(fork_fields),
        "        )",
        "        forked = await self._client.thread_fork(thread_id, params)",
        "        return AsyncThread(self, forked.thread.id)",
        "",
        "    async def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:",
        "        await self._ensure_initialized()",
        "        return await self._client.thread_archive(thread_id)",
        "",
        "    async def thread_unarchive(self, thread_id: str) -> AsyncThread:",
        "        await self._ensure_initialized()",
        "        unarchived = await self._client.thread_unarchive(thread_id)",
        "        return AsyncThread(self, unarchived.thread.id)",
    ]
    return "\n".join(lines)


def _render_thread_block(
    turn_fields: list[PublicFieldSpec],
) -> str:
    lines = [
        "    def turn(",
        "        self,",
        "        input: Input,",
        "        *,",
        *_kw_signature_lines(turn_fields),
        "    ) -> TurnHandle:",
        "        wire_input = _to_wire_input(input)",
        "        params = TurnStartParams(",
        "            thread_id=self.id,",
        "            input=wire_input,",
        *_model_arg_lines(turn_fields),
        "        )",
        "        turn = self._client.turn_start(self.id, wire_input, params=params)",
        "        return TurnHandle(self._client, self.id, turn.turn.id)",
    ]
    return "\n".join(lines)


def _render_async_thread_block(
    turn_fields: list[PublicFieldSpec],
) -> str:
    lines = [
        "    async def turn(",
        "        self,",
        "        input: Input,",
        "        *,",
        *_kw_signature_lines(turn_fields),
        "    ) -> AsyncTurnHandle:",
        "        await self._codex._ensure_initialized()",
        "        wire_input = _to_wire_input(input)",
        "        params = TurnStartParams(",
        "            thread_id=self.id,",
        "            input=wire_input,",
        *_model_arg_lines(turn_fields),
        "        )",
        "        turn = await self._codex._client.turn_start(",
        "            self.id,",
        "            wire_input,",
        "            params=params,",
        "        )",
        "        return AsyncTurnHandle(self._codex, self.id, turn.turn.id)",
    ]
    return "\n".join(lines)


def generate_public_api_flat_methods() -> None:
    src_dir = sdk_root() / "src"
    public_api_path = src_dir / "codex_app_server" / "api.py"
    if not public_api_path.exists():
        # PR2 can run codegen before the ergonomic public API layer is added.
        return
    src_dir_str = str(src_dir)
    if src_dir_str not in sys.path:
        sys.path.insert(0, src_dir_str)

    thread_start_fields = _load_public_fields(
        "codex_app_server.generated.v2_all",
        "ThreadStartParams",
    )
    thread_list_fields = _load_public_fields(
        "codex_app_server.generated.v2_all",
        "ThreadListParams",
    )
    thread_resume_fields = _load_public_fields(
        "codex_app_server.generated.v2_all",
        "ThreadResumeParams",
        exclude={"thread_id"},
    )
    thread_fork_fields = _load_public_fields(
        "codex_app_server.generated.v2_all",
        "ThreadForkParams",
        exclude={"thread_id"},
    )
    turn_start_fields = _load_public_fields(
        "codex_app_server.generated.v2_all",
        "TurnStartParams",
        exclude={"thread_id", "input"},
    )

    source = public_api_path.read_text()
    source = _replace_generated_block(
        source,
        "Codex.flat_methods",
        _render_codex_block(
            thread_start_fields,
            thread_list_fields,
            thread_resume_fields,
            thread_fork_fields,
        ),
    )
    source = _replace_generated_block(
        source,
        "AsyncCodex.flat_methods",
        _render_async_codex_block(
            thread_start_fields,
            thread_list_fields,
            thread_resume_fields,
            thread_fork_fields,
        ),
    )
    source = _replace_generated_block(
        source,
        "Thread.flat_methods",
        _render_thread_block(turn_start_fields),
    )
    source = _replace_generated_block(
        source,
        "AsyncThread.flat_methods",
        _render_async_thread_block(turn_start_fields),
    )
    public_api_path.write_text(source)


def generate_types(runtime_version: str | None = None) -> None:
    with tempfile.TemporaryDirectory(prefix="codex-python-schema-") as schema_root:
        schema_dir = Path(schema_root)
        _generate_json_schema_from_runtime(schema_dir, runtime_version)
        # v2_all is the authoritative generated surface.
        generate_v2_all(schema_dir)
        generate_notification_registry(schema_dir)
        generate_public_api_flat_methods()


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Single SDK maintenance entrypoint")
    subparsers = parser.add_subparsers(dest="command", required=True)

    generate_types_parser = subparsers.add_parser(
        "generate-types", help="Regenerate Python protocol-derived types"
    )
    generate_types_parser.add_argument(
        "--runtime-version",
        help=(
            "Runtime release version used to emit app-server JSON schema "
            "(defaults to sdk/python/_runtime_setup.py's pinned version)"
        ),
    )

    stage_sdk_parser = subparsers.add_parser(
        "stage-sdk",
        help="Stage a releasable SDK package pinned to a runtime version",
    )
    stage_sdk_parser.add_argument(
        "staging_dir",
        type=Path,
        help="Output directory for the staged SDK package",
    )
    stage_sdk_parser.add_argument(
        "--runtime-version",
        required=True,
        help=f"Pinned {RUNTIME_PKG_NAME} version for the staged SDK package",
    )
    stage_sdk_parser.add_argument(
        "--sdk-version",
        help="Version to write into the staged SDK package (defaults to sdk/python current version)",
    )

    stage_runtime_parser = subparsers.add_parser(
        "stage-runtime",
        help="Stage a releasable runtime package for the current platform",
    )
    stage_runtime_parser.add_argument(
        "staging_dir",
        type=Path,
        help="Output directory for the staged runtime package",
    )
    stage_runtime_parser.add_argument(
        "runtime_bundle_dir",
        type=Path,
        help="Directory containing the Codex runtime files to package for this platform",
    )
    stage_runtime_parser.add_argument(
        "--runtime-version",
        required=True,
        help="Version to write into the staged runtime package",
    )
    return parser


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    return build_parser().parse_args(list(argv) if argv is not None else None)


def default_cli_ops() -> CliOps:
    return CliOps(
        generate_types=generate_types,
        stage_python_sdk_package=stage_python_sdk_package,
        stage_python_runtime_package=stage_python_runtime_package,
        current_sdk_version=current_sdk_version,
    )


def run_command(args: argparse.Namespace, ops: CliOps) -> None:
    if args.command == "generate-types":
        ops.generate_types(args.runtime_version)
    elif args.command == "stage-sdk":
        ops.generate_types(None)
        ops.stage_python_sdk_package(
            args.staging_dir,
            args.sdk_version or ops.current_sdk_version(),
            args.runtime_version,
        )
    elif args.command == "stage-runtime":
        ops.stage_python_runtime_package(
            args.staging_dir,
            args.runtime_version,
            args.runtime_bundle_dir.resolve(),
        )


def main(argv: Sequence[str] | None = None, ops: CliOps | None = None) -> None:
    args = parse_args(argv)
    run_command(args, ops or default_cli_ops())
    print("Done.")


if __name__ == "__main__":
    main()
