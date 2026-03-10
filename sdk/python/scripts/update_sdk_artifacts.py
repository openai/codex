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


def _normalized_schema_bundle_text() -> str:
    schema = json.loads(schema_bundle_path().read_text())
    definitions = schema.get("definitions", {})
    if isinstance(definitions, dict):
        for definition in definitions.values():
            if isinstance(definition, dict):
                _flatten_string_enum_one_of(definition)
    return json.dumps(schema, indent=2, sort_keys=True) + "\n"


def generate_v2_all() -> None:
    out_path = sdk_root() / "src" / "codex_app_server" / "generated" / "v2_all.py"
    out_dir = out_path.parent
    old_package_dir = out_dir / "v2_all"
    if old_package_dir.exists():
        shutil.rmtree(old_package_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as td:
        normalized_bundle = Path(td) / schema_bundle_path().name
        normalized_bundle.write_text(_normalized_schema_bundle_text())
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
                "3.10",
                "--snake-case-field",
                "--allow-population-by-field-name",
                "--use-union-operator",
                "--reuse-model",
                "--disable-timestamp",
                "--use-double-quotes",
            ],
            cwd=sdk_root(),
        )
    _normalize_generated_timestamps(out_path)

def _notification_specs() -> list[tuple[str, str]]:
    server_notifications = json.loads((schema_root_dir() / "ServerNotification.json").read_text())
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
        if f"class {class_name}(" not in generated_source and f"{class_name} =" not in generated_source:
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
        "    ) -> Turn:",
        "        wire_input = _to_wire_input(input)",
        "        params = TurnStartParams(",
        "            thread_id=self.id,",
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
        "            thread_id=self.id,",
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


def generate_types() -> None:
    # v2_all is the authoritative generated surface.
    generate_v2_all()
    generate_notification_registry()
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
