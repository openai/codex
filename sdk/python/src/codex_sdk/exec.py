from __future__ import annotations

import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Generator, List, Optional

from .options import CancellationEvent

INTERNAL_ORIGINATOR_ENV = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE"
PYTHON_SDK_ORIGINATOR = "codex_sdk_py"


class CancelledError(Exception):
    """Raised when a turn is cancelled before completion."""


@dataclass
class CodexExecArgs:
    input: str
    base_url: Optional[str] = None
    api_key: Optional[str] = None
    thread_id: Optional[str] = None
    images: Optional[List[str]] = None
    model: Optional[str] = None
    sandbox_mode: Optional[str] = None
    working_directory: Optional[str] = None
    additional_directories: Optional[List[str]] = None
    skip_git_repo_check: bool = False
    output_schema_file: Optional[str] = None
    model_reasoning_effort: Optional[str] = None
    cancellation_event: Optional[CancellationEvent] = None
    network_access_enabled: Optional[bool] = None
    web_search_enabled: Optional[bool] = None
    approval_policy: Optional[str] = None


class CodexExec:
    def __init__(self, executable_path: Optional[str] = None, env: Optional[Dict[str, str]] = None) -> None:
        self.executable_path = executable_path or find_codex_path()
        self.env_override = env

    def run(self, args: CodexExecArgs) -> Generator[str, None, None]:
        cancel_event = args.cancellation_event
        if cancel_event and cancel_event.is_set():
            raise CancelledError("Turn cancelled before start")

        command_args: list[str] = ["exec", "--experimental-json"]
        if args.model:
            command_args.extend(["--model", args.model])
        if args.sandbox_mode:
            command_args.extend(["--sandbox", args.sandbox_mode])
        if args.working_directory:
            command_args.extend(["--cd", args.working_directory])
        if args.additional_directories:
            for extra_dir in args.additional_directories:
                command_args.extend(["--add-dir", extra_dir])
        if args.skip_git_repo_check:
            command_args.append("--skip-git-repo-check")
        if args.output_schema_file:
            command_args.extend(["--output-schema", args.output_schema_file])
        if args.model_reasoning_effort:
            command_args.extend(["--config", f'model_reasoning_effort="{args.model_reasoning_effort}"'])
        if args.network_access_enabled is not None:
            command_args.extend(
                ["--config", f"sandbox_workspace_write.network_access={str(args.network_access_enabled).lower()}"]
            )
        if args.web_search_enabled is not None:
            command_args.extend(["--config", f"features.web_search_request={str(args.web_search_enabled).lower()}"])
        if args.approval_policy:
            command_args.extend(["--config", f'approval_policy="{args.approval_policy}"'])
        if args.images:
            for image in args.images:
                command_args.extend(["--image", image])
        if args.thread_id:
            command_args.extend(["resume", args.thread_id])

        env = self._build_env(args)

        process: Optional[subprocess.Popen[str]] = None
        try:
            process = subprocess.Popen(
                [self.executable_path, *command_args],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
                env=env,
            )
            if not process.stdin or not process.stdout:
                raise RuntimeError("Failed to open stdio for Codex process")

            process.stdin.write(args.input)
            process.stdin.close()

            if cancel_event and cancel_event.is_set():
                raise CancelledError("Turn cancelled before first event")

            for raw_line in process.stdout:
                if cancel_event and cancel_event.is_set():
                    raise CancelledError("Turn cancelled")
                yield raw_line.rstrip("\r\n")

            process.wait()
            if cancel_event and cancel_event.is_set():
                raise CancelledError("Turn cancelled after process exit")
            if process.returncode:
                stderr_output = process.stderr.read() if process.stderr else ""
                raise RuntimeError(f"Codex Exec exited with code {process.returncode}: {stderr_output}")
        except CancelledError:
            if process:
                terminate_process(process)
            raise
        except Exception:
            if process:
                terminate_process(process)
            raise
        finally:
            if process:
                if process.poll() is None:
                    terminate_process(process)
                if process.stdout and not process.stdout.closed:
                    process.stdout.close()
                if process.stderr and not process.stderr.closed:
                    process.stderr.close()

    def _build_env(self, args: CodexExecArgs) -> Dict[str, str]:
        env: Dict[str, str] = {}
        if self.env_override is not None:
            env.update(self.env_override)
        else:
            env.update({key: value for key, value in os.environ.items() if value is not None})

        if INTERNAL_ORIGINATOR_ENV not in env:
            env[INTERNAL_ORIGINATOR_ENV] = PYTHON_SDK_ORIGINATOR
        if args.base_url:
            env["OPENAI_BASE_URL"] = args.base_url
        if args.api_key:
            env["CODEX_API_KEY"] = args.api_key
        return env


def terminate_process(process: subprocess.Popen[str]) -> None:
    try:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=2)
            except Exception:
                if process.poll() is None:
                    process.kill()
    except Exception:
        try:
            if process.poll() is None:
                process.kill()
        except Exception:
            pass


def find_codex_path() -> str:
    platform_name = sys.platform
    machine = platform.machine().lower()

    target_triple = None
    if platform_name.startswith("linux") or platform_name == "android":
        if machine in {"x86_64", "amd64"}:
            target_triple = "x86_64-unknown-linux-musl"
        elif machine in {"aarch64", "arm64"}:
            target_triple = "aarch64-unknown-linux-musl"
    elif platform_name == "darwin":
        if machine in {"x86_64", "amd64"}:
            target_triple = "x86_64-apple-darwin"
        elif machine in {"arm64", "aarch64"}:
            target_triple = "aarch64-apple-darwin"
    elif platform_name == "win32":
        if machine in {"x86_64", "amd64"}:
            target_triple = "x86_64-pc-windows-msvc"
        elif machine in {"arm64", "aarch64"}:
            target_triple = "aarch64-pc-windows-msvc"

    if target_triple is None:
        raise RuntimeError(f"Unsupported platform: {platform_name} ({machine})")

    package_root = Path(__file__).resolve().parent.parent
    vendor_root = package_root / "vendor" / target_triple / "codex"
    binary_name = "codex.exe" if platform_name == "win32" else "codex"
    binary_path = vendor_root / binary_name
    if not binary_path.exists():
        raise RuntimeError(
            f"Codex binary not found at {binary_path}. "
            "Install Codex or provide codex_path_override when creating the client."
        )
    return str(binary_path)
