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


def repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def sdk_root() -> Path:
    return repo_root() / "sdk" / "python"


def python_runtime_root() -> Path:
    return repo_root() / "sdk" / "python-runtime"


def schema_bundle_path() -> Path:
    return (
        repo_root()
        / "codex-rs"
        / "app-server-protocol"
        / "schema"
        / "json"
        / "codex_app_server_protocol.v2.schemas.json"
    )


def schema_root_dir() -> Path:
    return repo_root() / "codex-rs" / "app-server-protocol" / "schema" / "json"


def _is_windows() -> bool:
    return platform.system().lower().startswith("win")


def runtime_binary_name() -> str:
    return "codex.exe" if _is_windows() else "codex"


def staged_runtime_bin_path(root: Path) -> Path:
    return root / "src" / "codex_cli_bin" / "bin" / runtime_binary_name()


def run(cmd: list[str], cwd: Path) -> None:
    subprocess.run(cmd, cwd=str(cwd), check=True)


def run_python_module(module: str, args: list[str], cwd: Path) -> None:
    run([sys.executable, "-m", module, *args], cwd)


def run_capture(cmd: list[str], cwd: Path) -> str:
    result = subprocess.run(
        cmd,
        cwd=str(cwd),
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"Command failed ({result.returncode}): {' '.join(cmd)}\n"
            f"STDOUT:\n{result.stdout}\nSTDERR:\n{result.stderr}"
        )
    return result.stdout


def runtime_setup_path() -> Path:
    return sdk_root() / "_runtime_setup.py"


def pinned_runtime_version() -> str:
    spec = importlib.util.spec_from_file_location(
        "_runtime_setup", runtime_setup_path()
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Failed to load runtime setup module: {runtime_setup_path()}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.pinned_runtime_version()  # type: ignore[no-any-return]


def runtime_git_ref(version: str) -> str:
    return f"rust-v{version}"


def pinned_runtime_git_ref() -> str:
    return runtime_git_ref(pinned_runtime_version())


def ensure_git_ref_available(git_ref: str) -> None:
    result = subprocess.run(
        ["git", "rev-parse", "--verify", git_ref],
        cwd=str(repo_root()),
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode == 0:
        return
    run(["git", "fetch", "origin", "tag", git_ref, "--depth=1"], repo_root())


def read_git_file(git_ref: str, repo_path: str) -> str:
    return run_capture(["git", "show", f"{git_ref}:{repo_path}"], repo_root())


def materialize_schema_files_from_git_ref(git_ref: str, out_dir: Path) -> tuple[Path, Path]:
    out_dir.mkdir(parents=True, exist_ok=True)

    schema_bundle = out_dir / "codex_app_server_protocol.v2.schemas.json"
    schema_bundle.write_text(
        read_git_file(
            git_ref,
            "codex-rs/app-server-protocol/schema/json/"
            "codex_app_server_protocol.v2.schemas.json",
        )
    )

    server_notification = out_dir / "ServerNotification.json"
    server_notification.write_text(
        read_git_file(
            git_ref,
            "codex-rs/app-server-protocol/schema/json/ServerNotification.json",
        )
    )

    return schema_bundle, server_notification


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


def _rewrite_sdk_runtime_dependency(pyproject_text: str, runtime_version: str) -> str:
    match = re.search(r"^dependencies = \[(.*?)\]$", pyproject_text, flags=re.MULTILINE)
    if match is None:
        raise RuntimeError(
            "Could not find dependencies array in sdk/python/pyproject.toml"
        )

    raw_items = [item.strip() for item in match.group(1).split(",") if item.strip()]
    raw_items = [item for item in raw_items if "codex-cli-bin" not in item]
    raw_items.append(f'"codex-cli-bin=={runtime_version}"')
    replacement = "dependencies = [\n  " + ",\n  ".join(raw_items) + ",\n]"
    return pyproject_text[: match.start()] + replacement + pyproject_text[match.end() :]


def stage_python_sdk_package(
    staging_dir: Path, sdk_version: str, runtime_version: str
) -> Path:
    _copy_package_tree(sdk_root(), staging_dir)
    sdk_bin_dir = staging_dir / "src" / "codex_app_server" / "bin"
    if sdk_bin_dir.exists():
        shutil.rmtree(sdk_bin_dir)

    pyproject_path = staging_dir / "pyproject.toml"
    pyproject_text = pyproject_path.read_text()
    pyproject_text = _rewrite_project_version(pyproject_text, sdk_version)
    pyproject_text = _rewrite_sdk_runtime_dependency(pyproject_text, runtime_version)
    pyproject_path.write_text(pyproject_text)
    return staging_dir


def stage_python_runtime_package(
    staging_dir: Path, runtime_version: str, binary_path: Path
) -> Path:
    _copy_package_tree(python_runtime_root(), staging_dir)

    pyproject_path = staging_dir / "pyproject.toml"
    pyproject_path.write_text(
        _rewrite_project_version(pyproject_path.read_text(), runtime_version)
    )

    out_bin = staged_runtime_bin_path(staging_dir)
    out_bin.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(binary_path, out_bin)
    if not _is_windows():
        out_bin.chmod(
            out_bin.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH
        )
    return staging_dir


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


def _normalized_schema_bundle_text(schema_bundle: Path | None = None) -> str:
    bundle = schema_bundle or schema_bundle_path()
    schema = json.loads(bundle.read_text())
    definitions = schema.get("definitions", {})
    if isinstance(definitions, dict):
        for definition in definitions.values():
            if isinstance(definition, dict):
                _flatten_string_enum_one_of(definition)
    # Normalize the schema into something datamodel-code-generator can map to
    # stable class names instead of anonymous numbered helpers.
    _annotate_schema(schema)
    return json.dumps(schema, indent=2, sort_keys=True) + "\n"


def generate_v2_all(schema_bundle: Path | None = None) -> None:
    out_path = sdk_root() / "src" / "codex_app_server" / "generated" / "v2_all.py"
    out_dir = out_path.parent
    old_package_dir = out_dir / "v2_all"
    if old_package_dir.exists():
        shutil.rmtree(old_package_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    bundle = schema_bundle or schema_bundle_path()
    with tempfile.TemporaryDirectory() as td:
        normalized_bundle = Path(td) / bundle.name
        normalized_bundle.write_text(_normalized_schema_bundle_text(bundle))
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


def _notification_specs(
    server_notification_schema: Path | None = None,
) -> list[tuple[str, str]]:
    server_notification_path = server_notification_schema or (
        schema_root_dir() / "ServerNotification.json"
    )
    server_notifications = json.loads(
        server_notification_path.read_text()
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


def generate_notification_registry(
    server_notification_schema: Path | None = None,
) -> None:
    out = (
        sdk_root()
        / "src"
        / "codex_app_server"
        / "generated"
        / "notification_registry.py"
    )
    specs = _notification_specs(server_notification_schema)
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
    generate_types: Callable[[], None]
    generate_types_for_pinned_runtime: Callable[[str | None], None]
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

    original_source = public_api_path.read_text()
    source = _replace_generated_block(
        original_source,
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
    if source == original_source:
        return
    public_api_path.write_text(source)


def generate_types(
    schema_bundle: Path | None = None,
    server_notification_schema: Path | None = None,
) -> None:
    # v2_all is the authoritative generated surface.
    generate_v2_all(schema_bundle)
    generate_notification_registry(server_notification_schema)
    generate_public_api_flat_methods()


def generate_types_for_pinned_runtime(git_ref: str | None = None) -> None:
    pinned_ref = git_ref or pinned_runtime_git_ref()
    ensure_git_ref_available(pinned_ref)
    with tempfile.TemporaryDirectory(prefix="codex-python-pinned-schema-") as temp_root:
        schema_bundle, server_notification_schema = materialize_schema_files_from_git_ref(
            pinned_ref,
            Path(temp_root),
        )
        generate_types(
            schema_bundle=schema_bundle,
            server_notification_schema=server_notification_schema,
        )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Single SDK maintenance entrypoint")
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser(
        "generate-types", help="Regenerate Python protocol-derived types"
    )
    pinned_types_parser = subparsers.add_parser(
        "generate-types-for-pinned-runtime",
        help="Regenerate Python protocol-derived types from the pinned runtime version",
    )
    pinned_types_parser.add_argument(
        "--git-ref",
        help="Optional git ref to source vendored schema files from",
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
        help="Pinned codex-cli-bin version for the staged SDK package",
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
        "runtime_binary",
        type=Path,
        help="Path to the codex binary to package for this platform",
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
        generate_types_for_pinned_runtime=generate_types_for_pinned_runtime,
        stage_python_sdk_package=stage_python_sdk_package,
        stage_python_runtime_package=stage_python_runtime_package,
        current_sdk_version=current_sdk_version,
    )


def run_command(args: argparse.Namespace, ops: CliOps) -> None:
    if args.command == "generate-types":
        ops.generate_types()
    elif args.command == "generate-types-for-pinned-runtime":
        ops.generate_types_for_pinned_runtime(args.git_ref)
    elif args.command == "stage-sdk":
        ops.generate_types_for_pinned_runtime(runtime_git_ref(args.runtime_version))
        ops.stage_python_sdk_package(
            args.staging_dir,
            args.sdk_version or ops.current_sdk_version(),
            args.runtime_version,
        )
    elif args.command == "stage-runtime":
        ops.stage_python_runtime_package(
            args.staging_dir,
            args.runtime_version,
            args.runtime_binary.resolve(),
        )


def main(argv: Sequence[str] | None = None, ops: CliOps | None = None) -> None:
    args = parse_args(argv)
    run_command(args, ops or default_cli_ops())
    print("Done.")


if __name__ == "__main__":
    main()
