from __future__ import annotations

import asyncio
import json
import math
import os
import platform
import re
import shutil
from dataclasses import dataclass
from pathlib import Path
from typing import AsyncGenerator, Mapping, Sequence

from .options import (
    ApprovalMode,
    CodexConfigObject,
    CodexConfigValue,
    ModelReasoningEffort,
    SandboxMode,
    WebSearchMode,
)

INTERNAL_ORIGINATOR_ENV = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE"
PYTHON_SDK_ORIGINATOR = "codex_sdk_py"

_TOML_BARE_KEY = re.compile(r"^[A-Za-z0-9_-]+$")


@dataclass(slots=True)
class CodexExecArgs:
    input: str
    base_url: str | None = None
    api_key: str | None = None
    thread_id: str | None = None
    images: Sequence[str] | None = None
    model: str | None = None
    sandbox_mode: SandboxMode | None = None
    working_directory: str | None = None
    additional_directories: Sequence[str] | None = None
    skip_git_repo_check: bool = False
    output_schema_file: str | None = None
    model_reasoning_effort: ModelReasoningEffort | None = None
    signal: asyncio.Event | None = None
    network_access_enabled: bool | None = None
    web_search_mode: WebSearchMode | None = None
    web_search_enabled: bool | None = None
    approval_policy: ApprovalMode | None = None


class CodexExec:
    def __init__(
        self,
        executable_path: str | None = None,
        env: dict[str, str] | None = None,
        config_overrides: CodexConfigObject | None = None,
    ) -> None:
        self._executable_path = executable_path or find_codex_path()
        self._env_override = env
        self._config_overrides = config_overrides

    async def run(self, args: CodexExecArgs) -> AsyncGenerator[str, None]:
        if args.signal is not None and args.signal.is_set():
            raise asyncio.CancelledError("Codex execution aborted before start")

        command_args = self._build_command_args(args)
        env = self._build_env(args)

        process = await asyncio.create_subprocess_exec(
            self._executable_path,
            *command_args,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            env=env,
        )

        if process.stdin is None:
            await _terminate_process(process)
            raise RuntimeError("Child process has no stdin")
        if process.stdout is None:
            await _terminate_process(process)
            raise RuntimeError("Child process has no stdout")

        process.stdin.write(args.input.encode("utf-8"))
        await process.stdin.drain()
        process.stdin.close()
        if hasattr(process.stdin, "wait_closed"):
            await process.stdin.wait_closed()

        stderr_chunks = bytearray()
        stderr_task = asyncio.create_task(_collect_stream(process.stderr, stderr_chunks))

        try:
            while True:
                line = await _readline_with_signal(process.stdout, process, args.signal)
                if line == b"":
                    break
                yield line.decode("utf-8").rstrip("\r\n")

            returncode = await _wait_with_signal(process, args.signal)
            await stderr_task
            if returncode != 0:
                detail = f"code {returncode}"
                stderr_text = stderr_chunks.decode("utf-8")
                raise RuntimeError(f"Codex Exec exited with {detail}: {stderr_text}")
        finally:
            await stderr_task
            if process.returncode is None:
                await _terminate_process(process)

    def _build_command_args(self, args: CodexExecArgs) -> list[str]:
        command_args: list[str] = ["exec", "--experimental-json"]

        if self._config_overrides:
            for override in serialize_config_overrides(self._config_overrides):
                command_args.extend(["--config", override])

        if args.model:
            command_args.extend(["--model", args.model])
        if args.sandbox_mode:
            command_args.extend(["--sandbox", args.sandbox_mode])
        if args.working_directory:
            command_args.extend(["--cd", args.working_directory])
        if args.additional_directories:
            for directory in args.additional_directories:
                command_args.extend(["--add-dir", directory])
        if args.skip_git_repo_check:
            command_args.append("--skip-git-repo-check")
        if args.output_schema_file:
            command_args.extend(["--output-schema", args.output_schema_file])
        if args.model_reasoning_effort:
            command_args.extend(
                ["--config", f'model_reasoning_effort="{args.model_reasoning_effort}"']
            )
        if args.network_access_enabled is not None:
            value = "true" if args.network_access_enabled else "false"
            command_args.extend(["--config", f"sandbox_workspace_write.network_access={value}"])
        if args.web_search_mode:
            command_args.extend(["--config", f'web_search="{args.web_search_mode}"'])
        elif args.web_search_enabled is True:
            command_args.extend(["--config", 'web_search="live"'])
        elif args.web_search_enabled is False:
            command_args.extend(["--config", 'web_search="disabled"'])
        if args.approval_policy:
            command_args.extend(["--config", f'approval_policy="{args.approval_policy}"'])

        if args.thread_id:
            command_args.extend(["resume", args.thread_id])

        if args.images:
            for image in args.images:
                command_args.extend(["--image", image])

        return command_args

    def _build_env(self, args: CodexExecArgs) -> dict[str, str]:
        env: dict[str, str]
        if self._env_override is not None:
            env = dict(self._env_override)
        else:
            env = {k: v for k, v in os.environ.items()}

        if INTERNAL_ORIGINATOR_ENV not in env:
            env[INTERNAL_ORIGINATOR_ENV] = PYTHON_SDK_ORIGINATOR
        if args.base_url:
            env["OPENAI_BASE_URL"] = args.base_url
        if args.api_key:
            env["CODEX_API_KEY"] = args.api_key
        return env


async def _collect_stream(
    stream: asyncio.StreamReader | None,
    into: bytearray,
) -> None:
    if stream is None:
        return
    while True:
        chunk = await stream.read(4096)
        if not chunk:
            return
        into.extend(chunk)


async def _readline_with_signal(
    stream: asyncio.StreamReader,
    process: asyncio.subprocess.Process,
    signal: asyncio.Event | None,
) -> bytes:
    if signal is None:
        return await stream.readline()

    read_task = asyncio.create_task(stream.readline())
    signal_task = asyncio.create_task(signal.wait())
    done, pending = await asyncio.wait(
        {read_task, signal_task},
        return_when=asyncio.FIRST_COMPLETED,
    )
    for task in pending:
        task.cancel()
    if signal_task in done:
        read_task.cancel()
        await asyncio.gather(read_task, return_exceptions=True)
        await _terminate_process(process)
        raise asyncio.CancelledError("Codex execution aborted")
    signal_task.cancel()
    return await read_task


async def _wait_with_signal(
    process: asyncio.subprocess.Process,
    signal: asyncio.Event | None,
) -> int:
    if signal is None:
        return await process.wait()

    wait_task = asyncio.create_task(process.wait())
    signal_task = asyncio.create_task(signal.wait())
    done, pending = await asyncio.wait(
        {wait_task, signal_task},
        return_when=asyncio.FIRST_COMPLETED,
    )
    for task in pending:
        task.cancel()
    if signal_task in done:
        wait_task.cancel()
        await asyncio.gather(wait_task, return_exceptions=True)
        await _terminate_process(process)
        raise asyncio.CancelledError("Codex execution aborted")
    signal_task.cancel()
    return await wait_task


async def _terminate_process(process: asyncio.subprocess.Process) -> None:
    if process.returncode is not None:
        return
    process.terminate()
    try:
        await asyncio.wait_for(process.wait(), timeout=1.0)
    except asyncio.TimeoutError:
        process.kill()
        await process.wait()


def serialize_config_overrides(config_overrides: CodexConfigObject) -> list[str]:
    overrides: list[str] = []
    flatten_config_overrides(config_overrides, "", overrides)
    return overrides


def flatten_config_overrides(
    value: CodexConfigValue,
    prefix: str,
    overrides: list[str],
) -> None:
    if not _is_plain_object(value):
        if prefix:
            overrides.append(f"{prefix}={to_toml_value(value, prefix)}")
            return
        raise ValueError("Codex config overrides must be a plain object")

    entries = list(value.items())
    if not prefix and len(entries) == 0:
        return

    if prefix and len(entries) == 0:
        overrides.append(f"{prefix}={{}}")
        return

    for key, child in entries:
        if key == "":
            raise ValueError("Codex config override keys must be non-empty strings")
        path = f"{prefix}.{key}" if prefix else key
        if _is_plain_object(child):
            flatten_config_overrides(child, path, overrides)
        else:
            overrides.append(f"{path}={to_toml_value(child, path)}")


def to_toml_value(value: CodexConfigValue, path: str) -> str:
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        if isinstance(value, float) and not math.isfinite(value):
            raise ValueError(f"Codex config override at {path} must be a finite number")
        return str(value)
    if isinstance(value, list):
        rendered = [to_toml_value(item, f"{path}[{idx}]") for idx, item in enumerate(value)]
        return f"[{', '.join(rendered)}]"
    if _is_plain_object(value):
        parts: list[str] = []
        for key, child in value.items():
            if key == "":
                raise ValueError("Codex config override keys must be non-empty strings")
            parts.append(f"{_format_toml_key(key)} = {to_toml_value(child, f'{path}.{key}')}")
        return "{" + ", ".join(parts) + "}"
    raise ValueError(f"Unsupported Codex config override value at {path}: {type(value).__name__}")


def _format_toml_key(key: str) -> str:
    if _TOML_BARE_KEY.fullmatch(key):
        return key
    return json.dumps(key)


def _is_plain_object(value: object) -> bool:
    return isinstance(value, Mapping)


def find_codex_path() -> str:
    target_triple = _platform_target_triple()
    script_dir = Path(__file__).resolve().parent
    vendor_root = script_dir.parent / "vendor"
    binary_name = "codex.exe" if os.name == "nt" else "codex"
    binary_path = vendor_root / target_triple / "codex" / binary_name
    if binary_path.exists():
        return str(binary_path)

    on_path = shutil.which("codex")
    if on_path:
        return on_path
    raise RuntimeError(
        "Unable to locate codex binary. "
        "Set codex_path_override or ensure 'codex' is available on PATH."
    )


def _platform_target_triple() -> str:
    system = platform.system().lower()
    machine = platform.machine().lower()

    if system == "linux":
        if machine in {"x86_64", "amd64"}:
            return "x86_64-unknown-linux-musl"
        if machine in {"arm64", "aarch64"}:
            return "aarch64-unknown-linux-musl"
    if system == "darwin":
        if machine in {"x86_64", "amd64"}:
            return "x86_64-apple-darwin"
        if machine in {"arm64", "aarch64"}:
            return "aarch64-apple-darwin"
    if system == "windows":
        if machine in {"x86_64", "amd64"}:
            return "x86_64-pc-windows-msvc"
        if machine in {"arm64", "aarch64"}:
            return "aarch64-pc-windows-msvc"
    raise RuntimeError(f"Unsupported platform: {system} ({machine})")
