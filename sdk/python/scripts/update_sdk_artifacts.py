#!/usr/bin/env python3
from __future__ import annotations

import argparse
import importlib
import json
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import types
import typing
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, get_args, get_origin


def repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def sdk_root() -> Path:
    return repo_root() / "sdk" / "python"


def schema_dir() -> Path:
    return repo_root() / "codex-rs" / "app-server-protocol" / "schema" / "json" / "v2"


def schema_root_dir() -> Path:
    return repo_root() / "codex-rs" / "app-server-protocol" / "schema" / "json"


def _is_windows() -> bool:
    return platform.system().lower().startswith("win")


def pinned_bin_path() -> Path:
    name = "codex.exe" if _is_windows() else "codex"
    return sdk_root() / "bin" / name


def bundled_platform_bin_path(platform_key: str) -> Path:
    exe = "codex.exe" if platform_key.startswith("windows") else "codex"
    return sdk_root() / "src" / "codex_app_server" / "bin" / platform_key / exe


PLATFORMS: dict[str, tuple[list[str], list[str]]] = {
    "darwin-arm64": (["darwin", "apple-darwin", "macos"], ["aarch64", "arm64"]),
    "darwin-x64": (["darwin", "apple-darwin", "macos"], ["x86_64", "amd64", "x64"]),
    "linux-arm64": (["linux", "unknown-linux", "musl", "gnu"], ["aarch64", "arm64"]),
    "linux-x64": (["linux", "unknown-linux", "musl", "gnu"], ["x86_64", "amd64", "x64"]),
    "windows-arm64": (["windows", "pc-windows", "win", "msvc", "gnu"], ["aarch64", "arm64"]),
    "windows-x64": (["windows", "pc-windows", "win", "msvc", "gnu"], ["x86_64", "amd64", "x64"]),
}


def run(cmd: list[str], cwd: Path) -> None:
    subprocess.run(cmd, cwd=str(cwd), check=True)


def run_python_module(module: str, args: list[str], cwd: Path) -> None:
    run([sys.executable, "-m", module, *args], cwd)


def platform_tokens() -> tuple[list[str], list[str]]:
    sys_name = platform.system().lower()
    machine = platform.machine().lower()

    if sys_name == "darwin":
        os_tokens = ["darwin", "apple-darwin", "macos"]
    elif sys_name == "linux":
        os_tokens = ["linux", "unknown-linux", "musl", "gnu"]
    elif sys_name.startswith("win"):
        os_tokens = ["windows", "pc-windows", "win", "msvc", "gnu"]
    else:
        raise RuntimeError(f"Unsupported OS: {sys_name}")

    if machine in {"arm64", "aarch64"}:
        arch_tokens = ["aarch64", "arm64"]
    elif machine in {"x86_64", "amd64"}:
        arch_tokens = ["x86_64", "amd64", "x64"]
    else:
        raise RuntimeError(f"Unsupported architecture: {machine}")

    return os_tokens, arch_tokens


def pick_release(channel: str) -> dict[str, Any]:
    releases = json.loads(
        subprocess.check_output(["gh", "api", "repos/openai/codex/releases?per_page=50"], text=True)
    )
    if channel == "stable":
        candidates = [r for r in releases if not r.get("prerelease") and not r.get("draft")]
    else:
        candidates = [r for r in releases if r.get("prerelease") and not r.get("draft")]
    if not candidates:
        raise RuntimeError(f"No {channel} release found")
    return candidates[0]


def pick_asset(release: dict[str, Any], os_tokens: list[str], arch_tokens: list[str]) -> dict[str, Any]:
    scored: list[tuple[int, dict[str, Any]]] = []
    for asset in release.get("assets", []):
        name = (asset.get("name") or "").lower()

        # Accept only primary codex cli artifacts.
        if not (name.startswith("codex-") or name == "codex"):
            continue
        if name.startswith("codex-responses") or name.startswith("codex-command-runner") or name.startswith("codex-windows-sandbox") or name.startswith("codex-npm"):
            continue
        if not (name.endswith(".tar.gz") or name.endswith(".zip")):
            continue

        os_score = sum(1 for t in os_tokens if t in name)
        arch_score = sum(1 for t in arch_tokens if t in name)
        if os_score == 0 or arch_score == 0:
            continue

        score = os_score * 10 + arch_score
        scored.append((score, asset))

    if not scored:
        raise RuntimeError("Could not find matching codex CLI asset for this platform")

    scored.sort(key=lambda x: x[0], reverse=True)
    return scored[0][1]


def download(url: str, out: Path) -> None:
    req = urllib.request.Request(url, headers={"User-Agent": "codex-python-sdk-updater"})
    with urllib.request.urlopen(req) as resp, out.open("wb") as f:
        shutil.copyfileobj(resp, f)


def extract_codex_binary(archive: Path, out_bin: Path) -> None:
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        if archive.name.endswith(".tar.gz"):
            with tarfile.open(archive, "r:gz") as tar:
                tar.extractall(tmp)
        elif archive.name.endswith(".zip"):
            with zipfile.ZipFile(archive) as zf:
                zf.extractall(tmp)
        else:
            raise RuntimeError(f"Unsupported archive format: {archive}")

        preferred_names = {"codex.exe", "codex"}
        candidates = [
            p for p in tmp.rglob("*") if p.is_file() and (p.name.lower() in preferred_names or p.name.lower().startswith("codex-"))
        ]
        if not candidates:
            raise RuntimeError("No codex binary found in release archive")

        candidates.sort(key=lambda p: (p.name.lower() not in preferred_names, p.name.lower()))

        out_bin.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(candidates[0], out_bin)
        if not _is_windows():
            out_bin.chmod(out_bin.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def _download_asset_to_binary(release: dict[str, Any], os_tokens: list[str], arch_tokens: list[str], out_bin: Path) -> None:
    asset = pick_asset(release, os_tokens, arch_tokens)
    print(f"Asset: {asset.get('name')} -> {out_bin}")
    with tempfile.TemporaryDirectory() as td:
        archive = Path(td) / (asset.get("name") or "codex-release.tar.gz")
        download(asset["browser_download_url"], archive)
        extract_codex_binary(archive, out_bin)


def update_binary(channel: str) -> None:
    if shutil.which("gh") is None:
        raise RuntimeError("GitHub CLI (`gh`) is required to download release binaries")

    release = pick_release(channel)
    os_tokens, arch_tokens = platform_tokens()
    print(f"Release: {release.get('tag_name')} ({channel})")

    # refresh current platform in bundled runtime location
    current_key = next((k for k, v in PLATFORMS.items() if v == (os_tokens, arch_tokens)), None)
    out = bundled_platform_bin_path(current_key) if current_key else pinned_bin_path()
    _download_asset_to_binary(release, os_tokens, arch_tokens, out)
    print(f"Pinned binary updated: {out}")


def bundle_all_platform_binaries(channel: str) -> None:
    if shutil.which("gh") is None:
        raise RuntimeError("GitHub CLI (`gh`) is required to download release binaries")

    release = pick_release(channel)
    print(f"Release: {release.get('tag_name')} ({channel})")
    for platform_key, (os_tokens, arch_tokens) in PLATFORMS.items():
        _download_asset_to_binary(release, os_tokens, arch_tokens, bundled_platform_bin_path(platform_key))
    print("Bundled all platform binaries.")


def generate_v2_all() -> None:
    out_dir = sdk_root() / "src" / "codex_app_server" / "generated" / "v2_all"
    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    run_python_module(
        "datamodel_code_generator",
        [
            "--input",
            str(schema_dir()),
            "--input-file-type",
            "jsonschema",
            "--output",
            str(out_dir),
            "--output-model-type",
            "pydantic_v2.BaseModel",
            "--target-python-version",
            "3.10",
            "--use-double-quotes",
        ],
        cwd=sdk_root(),
    )
    _normalize_generated_timestamps(out_dir)
    (out_dir / "__init__.py").touch()

def _notification_specs() -> list[tuple[str, str]]:
    server_notifications = json.loads((schema_root_dir() / "ServerNotification.json").read_text())
    one_of = server_notifications.get("oneOf", [])

    specs: list[tuple[str, str]] = []
    v2_dir = sdk_root() / "src" / "codex_app_server" / "generated" / "v2_all"

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
        if not (v2_dir / f"{class_name}.py").exists():
            # Skip schema variants that are not emitted into the generated v2 surface.
            continue
        specs.append((method, class_name))

    specs.sort()
    return specs


def generate_notification_registry() -> None:
    out = sdk_root() / "src" / "codex_app_server" / "generated" / "notification_registry.py"
    specs = _notification_specs()
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
        lines.append(f"from .v2_all.{class_name} import {class_name}")
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


def _event_msg_types() -> list[str]:
    schema = json.loads((schema_root_dir() / "codex_app_server_protocol.schemas.json").read_text())
    definitions = schema.get("definitions", {})
    event_msg = definitions.get("EventMsg", {})
    one_of = event_msg.get("oneOf", [])

    types: set[str] = set()
    for variant in one_of:
        props = variant.get("properties", {})
        type_meta = props.get("type", {})
        enum_values = type_meta.get("enum", [])
        if len(enum_values) != 1:
            continue
        value = enum_values[0]
        if isinstance(value, str):
            types.add(value)

    return sorted(types)


def generate_codex_event_types() -> None:
    out = sdk_root() / "src" / "codex_app_server" / "generated" / "codex_event_types.py"
    event_types = _event_msg_types()

    literal_values = ", ".join(repr(event_type) for event_type in event_types)
    event_type_alias = f"Literal[{literal_values}]" if literal_values else "str"

    lines = [
        "# Auto-generated by scripts/update_sdk_artifacts.py",
        "# DO NOT EDIT MANUALLY.",
        "",
        "from __future__ import annotations",
        "",
        "from typing import Any, Literal",
        "",
        "from pydantic import BaseModel, ConfigDict",
        "",
        f"CodexEventType = {event_type_alias}",
        "",
        "",
        "class CodexEventMessage(BaseModel):",
        "    model_config = ConfigDict(extra=\"allow\")",
        "    type: CodexEventType | str",
        "",
        "",
        "class CodexEventNotification(BaseModel):",
        "    id: str | None = None",
        "    conversationId: str | None = None",
        "    msg: CodexEventMessage | dict[str, Any]",
        "",
    ]
    out.write_text("\n".join(lines))


def _normalize_generated_timestamps(root: Path) -> None:
    timestamp_re = re.compile(r"^#\s+timestamp:\s+.+$", flags=re.MULTILINE)
    for py_file in root.rglob("*.py"):
        content = py_file.read_text()
        normalized = timestamp_re.sub("#   timestamp: <normalized>", content)
        if normalized != content:
            py_file.write_text(normalized)


# ---- protocol_types.py generation ----
def load_schema(name: str) -> dict[str, Any]:
    return json.loads((schema_dir() / f"{name}.json").read_text())


def object_props(schema: dict[str, Any], node: dict[str, Any]) -> tuple[dict[str, Any], set[str]]:
    if "$ref" in node:
        ref = node["$ref"]
        if ref.startswith("#/definitions/"):
            key = ref.split("/")[-1]
            return object_props(schema, schema["definitions"][key])
        raise ValueError(f"unsupported ref: {ref}")
    return node.get("properties", {}), set(node.get("required", []))


def field_type(v: dict[str, Any]) -> str:
    if "$ref" in v:
        ref = v["$ref"]
        if ref.endswith("Thread"):
            return "ThreadObject"
        if ref.endswith("Turn"):
            return "TurnObject"
        if ref.endswith("ThreadTokenUsage"):
            return "ThreadTokenUsage"
        return "dict[str, Any]"
    if "anyOf" in v:
        non_null = [x for x in v["anyOf"] if x.get("type") != "null"]
        if len(non_null) == 1:
            return f"{field_type(non_null[0])} | None"
    t = v.get("type")
    if t == "string":
        return "str"
    if t == "integer":
        return "int"
    if t == "boolean":
        return "bool"
    if t == "array":
        if (v.get("items") or {}).get("$ref", "").endswith("Thread"):
            return "list[ThreadObject]"
        if (v.get("items") or {}).get("$ref", "").endswith("Turn"):
            return "list[TurnObject]"
        return "list[dict[str, Any]]"
    return "dict[str, Any]"


def render_typed_dict(name: str, props: dict[str, Any], req: set[str]) -> str:
    lines = [f"class {name}(TypedDict):"]
    if not props:
        lines.append("    pass")
        return "\n".join(lines)
    for k, v in props.items():
        t = field_type(v)
        if k in req:
            lines.append(f"    {k}: {t}")
        else:
            lines.append(f"    {k}: NotRequired[{t}]")
    return "\n".join(lines)


def generate_protocol_types() -> None:
    out = sdk_root() / "src" / "codex_app_server" / "generated" / "protocol_types.py"
    tsr = load_schema("ThreadStartResponse")
    turs = load_schema("TurnStartResponse")
    ttu = load_schema("ThreadTokenUsageUpdatedNotification")

    thread_props, thread_req = object_props(tsr, tsr["definitions"].get("Thread", {}))
    turn_props, turn_req = object_props(turs, turs["definitions"].get("Turn", {}))
    usage_props, usage_req = object_props(ttu, ttu["definitions"].get("ThreadTokenUsage", {}))

    roots = {
        "ThreadStartResponse": object_props(tsr, tsr),
        "TurnStartResponse": object_props(turs, turs),
        "ThreadTokenUsageUpdatedNotificationParams": object_props(ttu, ttu),
    }

    parts = [
        "from __future__ import annotations",
        "",
        "from typing import Any, NotRequired, TypedDict",
        "",
        "# Generated by scripts/update_sdk_artifacts.py",
        "",
        render_typed_dict("ThreadObject", thread_props, thread_req),
        "",
        render_typed_dict("TurnObject", turn_props, turn_req),
        "",
        render_typed_dict("ThreadTokenUsage", usage_props, usage_req),
        "",
    ]
    for name, (props, req) in roots.items():
        parts.append(render_typed_dict(name, props, req))
        parts.append("")

    out.write_text("\n".join(parts))


# ---- schema_types.py generation ----
TARGET_SCHEMAS = {
    "ThreadStartResponse": "ThreadStartResponse.json",
    "ThreadResumeResponse": "ThreadResumeResponse.json",
    "ThreadReadResponse": "ThreadReadResponse.json",
    "ThreadListResponse": "ThreadListResponse.json",
    "ThreadForkResponse": "ThreadForkResponse.json",
    "ThreadArchiveResponse": "ThreadArchiveResponse.json",
    "ThreadUnarchiveResponse": "ThreadUnarchiveResponse.json",
    "ThreadSetNameResponse": "ThreadSetNameResponse.json",
    "ThreadCompactStartResponse": "ThreadCompactStartResponse.json",
    "TurnStartResponse": "TurnStartResponse.json",
    "TurnSteerResponse": "TurnSteerResponse.json",
    "ModelListResponse": "ModelListResponse.json",
}


@dataclass(slots=True)
class FieldSpec:
    name: str
    annotation: str
    required: bool
    source_expr: str


@dataclass(slots=True)
class ClassSpec:
    name: str
    fields: list[FieldSpec]


def py_type_for_schema(schema: dict[str, Any], defs: dict[str, Any], nested: set[str]) -> tuple[str, str]:
    if "$ref" in schema:
        ref = schema["$ref"].split("/")[-1]
        if ref in nested:
            return ref, "object"
        rd = defs.get(ref, {})
        if rd.get("type") == "string":
            return "str", "scalar"
        if rd.get("type") == "integer":
            return "int", "scalar"
        if rd.get("type") == "boolean":
            return "bool", "scalar"
        return "Any", "scalar"
    t = schema.get("type")
    if t == "string":
        return "str", "scalar"
    if t == "integer":
        return "int", "scalar"
    if t == "boolean":
        return "bool", "scalar"
    if t == "array":
        item_t, _ = py_type_for_schema(schema.get("items", {}), defs, nested)
        return f"list[{item_t}]", "array"
    if t == "object":
        return "dict[str, Any]", "object"
    return "Any", "scalar"


def field_source(field_name: str, py_type: str, kind: str) -> str:
    g = f'payload.get("{field_name}")'
    if py_type == "str":
        return f"str({g} or '')"
    if py_type == "int":
        return f"int({g} or 0)"
    if py_type == "bool":
        return f"bool({g})"
    if kind == "array":
        return f"list({g} or [])"
    return g


def class_from_schema(name: str, schema: dict[str, Any], defs: dict[str, Any], nested: set[str]) -> ClassSpec:
    props = schema.get("properties", {})
    req = set(schema.get("required", []))
    fields: list[FieldSpec] = []
    for n, s in props.items():
        t, k = py_type_for_schema(s, defs, nested)
        fields.append(FieldSpec(name=n, annotation=t, required=n in req, source_expr=field_source(n, t, k)))
    return ClassSpec(name=name, fields=fields)


def generate_schema_types() -> None:
    out = sdk_root() / "src" / "codex_app_server" / "generated" / "schema_types.py"
    raw: dict[str, dict[str, Any]] = {}
    defs: dict[str, Any] = {}
    for cname, fname in TARGET_SCHEMAS.items():
        data = json.loads((schema_dir() / fname).read_text())
        raw[cname] = data
        defs.update(data.get("definitions", {}))

    nested = {"Thread", "Turn"}
    specs: list[ClassSpec] = []
    for n in sorted(nested):
        if defs.get(n):
            specs.append(class_from_schema(n, defs[n], defs, nested))
    for name, root in raw.items():
        specs.append(class_from_schema(name, root, defs, nested))

    parts: list[str] = [
        "# Auto-generated by scripts/update_sdk_artifacts.py",
        "# DO NOT EDIT MANUALLY.",
        "",
        "from __future__ import annotations",
        "",
        "from dataclasses import dataclass",
        "from typing import Any, TypedDict",
        "",
    ]
    for spec in specs:
        parts.append(f"class {spec.name}Dict(TypedDict, total=False):")
        if spec.fields:
            for f in spec.fields:
                parts.append(f"    {f.name}: {f.annotation}")
        else:
            parts.append("    pass")
        parts.append("")
        parts.append("@dataclass(slots=True, kw_only=True)")
        parts.append(f"class {spec.name}:")
        if spec.fields:
            for f in spec.fields:
                default = "" if f.required else " = None"
                parts.append(f"    {f.name}: {f.annotation}{default}")
        else:
            parts.append("    pass")
        parts.append("")
    out.write_text("\n".join(parts) + "\n")


TYPE_ALIAS_MAP: dict[tuple[str, str], str] = {
    ("codex_app_server.generated.v2_all.ThreadStartParams", "AskForApproval"): "AskForApproval",
    ("codex_app_server.generated.v2_all.ThreadStartParams", "Personality"): "Personality",
    ("codex_app_server.generated.v2_all.ThreadStartParams", "SandboxMode"): "SandboxMode",
    ("codex_app_server.generated.v2_all.ThreadListParams", "ThreadSortKey"): "ThreadSortKey",
    ("codex_app_server.generated.v2_all.ThreadListParams", "ThreadSourceKind"): "ThreadSourceKind",
    ("codex_app_server.generated.v2_all.ThreadResumeParams", "AskForApproval"): "ResumeAskForApproval",
    ("codex_app_server.generated.v2_all.ThreadResumeParams", "Personality"): "ResumePersonality",
    ("codex_app_server.generated.v2_all.ThreadResumeParams", "SandboxMode"): "ResumeSandboxMode",
    ("codex_app_server.generated.v2_all.ThreadForkParams", "AskForApproval"): "ForkAskForApproval",
    ("codex_app_server.generated.v2_all.ThreadForkParams", "SandboxMode"): "ForkSandboxMode",
    ("codex_app_server.generated.v2_all.TurnStartParams", "AskForApproval"): "TurnAskForApproval",
    ("codex_app_server.generated.v2_all.TurnStartParams", "Personality"): "TurnPersonality",
    ("codex_app_server.generated.v2_all.TurnStartParams", "ReasoningEffort"): "TurnReasoningEffort",
    ("codex_app_server.generated.v2_all.TurnStartParams", "SandboxPolicy"): "TurnSandboxPolicy",
    ("codex_app_server.generated.v2_all.TurnStartParams", "ReasoningSummary"): "TurnReasoningSummary",
}

FIELD_ANNOTATION_OVERRIDES: dict[str, str] = {
    # Keep public API typed without falling back to `Any`.
    "config": "JsonObject",
    "outputSchema": "JsonObject",
}


@dataclass(slots=True)
class PublicFieldSpec:
    wire_name: str
    py_name: str
    annotation: str
    required: bool


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
        alias = TYPE_ALIAS_MAP.get((annotation.__module__, annotation.__name__))
        if alias is not None:
            return alias
    return "Any"


def _camel_to_snake(name: str) -> str:
    head = re.sub(r"(.)([A-Z][a-z]+)", r"\1_\2", name)
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", head).lower()


def _load_public_fields(module_name: str, class_name: str, *, exclude: set[str] | None = None) -> list[PublicFieldSpec]:
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
                py_name=_camel_to_snake(name),
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


def _model_arg_lines(fields: list[PublicFieldSpec], *, indent: str = "            ") -> list[str]:
    return [f"{indent}{field.wire_name}={field.py_name}," for field in fields]


def _replace_generated_block(source: str, block_name: str, body: str) -> str:
    start_tag = f"    # BEGIN GENERATED: {block_name}"
    end_tag = f"    # END GENERATED: {block_name}"
    pattern = re.compile(
        rf"(?s){re.escape(start_tag)}\n.*?\n{re.escape(end_tag)}"
    )
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
        "            threadId=thread_id,",
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
        "            threadId=thread_id,",
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
        "            threadId=thread_id,",
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
        "            threadId=thread_id,",
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
        "    ) -> Turn:",
        "        wire_input = _to_wire_input(input)",
        "        params = TurnStartParams(",
        "            threadId=self.id,",
        "            input=wire_input,",
        *_model_arg_lines(turn_fields),
        "        )",
        "        turn = self._client.turn_start(self.id, wire_input, params=params)",
        "        return Turn(self._client, self.id, turn.turn.id)",
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
        "    ) -> AsyncTurn:",
        "        await self._codex._ensure_initialized()",
        "        wire_input = _to_wire_input(input)",
        "        params = TurnStartParams(",
        "            threadId=self.id,",
        "            input=wire_input,",
        *_model_arg_lines(turn_fields),
        "        )",
        "        turn = await self._codex._client.turn_start(",
        "            self.id,",
        "            wire_input,",
        "            params=params,",
        "        )",
        "        return AsyncTurn(self._codex, self.id, turn.turn.id)",
    ]
    return "\n".join(lines)


def generate_public_api_flat_methods() -> None:
    src_dir = sdk_root() / "src"
    public_api_path = src_dir / "codex_app_server" / "public_api.py"
    if not public_api_path.exists():
        # PR2 can run codegen before the ergonomic public API layer is added.
        return
    src_dir_str = str(src_dir)
    if src_dir_str not in sys.path:
        sys.path.insert(0, src_dir_str)

    thread_start_fields = _load_public_fields(
        "codex_app_server.generated.v2_all.ThreadStartParams",
        "ThreadStartParams",
    )
    thread_list_fields = _load_public_fields(
        "codex_app_server.generated.v2_all.ThreadListParams",
        "ThreadListParams",
    )
    thread_resume_fields = _load_public_fields(
        "codex_app_server.generated.v2_all.ThreadResumeParams",
        "ThreadResumeParams",
        exclude={"threadId"},
    )
    thread_fork_fields = _load_public_fields(
        "codex_app_server.generated.v2_all.ThreadForkParams",
        "ThreadForkParams",
        exclude={"threadId"},
    )
    turn_start_fields = _load_public_fields(
        "codex_app_server.generated.v2_all.TurnStartParams",
        "TurnStartParams",
        exclude={"threadId", "input"},
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


def generate_types() -> None:
    # v2_all is the authoritative generated surface.
    generate_v2_all()
    generate_protocol_types()
    generate_schema_types()
    generate_notification_registry()
    generate_codex_event_types()
    generate_public_api_flat_methods()


def main() -> None:
    parser = argparse.ArgumentParser(description="Single SDK maintenance entrypoint")
    parser.add_argument("--channel", choices=["stable", "alpha"], default="stable")
    parser.add_argument("--types-only", action="store_true", help="Regenerate types only (skip binary update)")
    parser.add_argument(
        "--bundle-all-platforms",
        action="store_true",
        help="Download and bundle codex binaries for all supported OS/arch targets",
    )
    args = parser.parse_args()

    if not args.types_only:
        if args.bundle_all_platforms:
            bundle_all_platform_binaries(args.channel)
        else:
            update_binary(args.channel)
    generate_types()
    print("Done.")


if __name__ == "__main__":
    main()
