from __future__ import annotations

import os
import platform
import shutil
import subprocess
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from .abort import AbortSignal
from .errors import AbortError, AuthRequiredError, CodexNotInstalledError, ThreadRunError
from .options import ApprovalMode, ModelReasoningEffort, SandboxMode

INTERNAL_ORIGINATOR_ENV = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE"
PYTHON_SDK_ORIGINATOR = "codex_sdk_py"


@dataclass
class CodexExecArgs:
    input: str
    base_url: str | None = None
    api_key: str | None = None
    thread_id: str | None = None
    images: list[str] | None = None
    model: str | None = None
    sandbox_mode: SandboxMode | None = None
    working_directory: str | None = None
    additional_directories: list[str] | None = None
    skip_git_repo_check: bool | None = None
    output_schema_file: str | None = None
    model_reasoning_effort: ModelReasoningEffort | None = None
    signal: object | None = None
    network_access_enabled: bool | None = None
    web_search_enabled: bool | None = None
    approval_policy: ApprovalMode | None = None


class CodexExec:
    def __init__(self, executable_path: str | None = None, env: dict[str, str] | None = None) -> None:
        self._executable_path = executable_path or find_codex_path()
        self._env_override = env

    def run(self, args: CodexExecArgs) -> Iterable[str]:
        if _is_aborted(args.signal):
            raise AbortError(_abort_reason(args.signal))

        command_args = _build_command_args(args)
        env = _build_env(self._env_override, args.base_url, args.api_key)
        _ensure_auth(env, args.api_key)

        try:
            process = subprocess.Popen(
                [self._executable_path, *command_args],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                env=env,
            )
        except FileNotFoundError as exc:
            raise _codex_not_installed_error() from exc

        if process.stdin is None or process.stdout is None or process.stderr is None:
            process.kill()
            raise ThreadRunError("Child process missing standard streams")

        process.stdin.write(args.input)
        process.stdin.close()

        stderr_chunks: list[str] = []

        def _read_stderr() -> None:
            data = process.stderr.read()
            if data:
                stderr_chunks.append(data)

        stderr_thread = threading.Thread(target=_read_stderr, daemon=True)
        stderr_thread.start()

        abort_event = threading.Event()
        abort_thread: threading.Thread | None = None
        if args.signal is not None:
            def _watch_abort() -> None:
                while not _is_aborted(args.signal):
                    if process.poll() is not None:
                        return
                    time.sleep(0.05)
                abort_event.set()
                _terminate_process(process)

            abort_thread = threading.Thread(target=_watch_abort, daemon=True)
            abort_thread.start()

        try:
            for raw_line in process.stdout:
                if abort_event.is_set():
                    raise AbortError(_abort_reason(args.signal))
                line = raw_line.rstrip("\n")
                if line:
                    yield line
            returncode = process.wait()
            stderr_thread.join(timeout=1)
            if abort_event.is_set():
                raise AbortError(_abort_reason(args.signal))
            if returncode != 0:
                stderr_text = "".join(stderr_chunks)
                raise ThreadRunError(
                    f"Codex Exec exited with code {returncode}: {stderr_text}"
                )
        finally:
            if abort_thread is not None:
                abort_thread.join(timeout=1)
            _terminate_process(process)


def _build_command_args(args: CodexExecArgs) -> list[str]:
    command_args = ["exec", "--experimental-json"]

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
        command_args.extend(["--config", f'model_reasoning_effort="{args.model_reasoning_effort}"'])
    if args.network_access_enabled is not None:
        value = str(args.network_access_enabled).lower()
        command_args.extend(["--config", f"sandbox_workspace_write.network_access={value}"])
    if args.web_search_enabled is not None:
        value = str(args.web_search_enabled).lower()
        command_args.extend(["--config", f"features.web_search_request={value}"])
    if args.approval_policy:
        command_args.extend(["--config", f'approval_policy="{args.approval_policy}"'])
    if args.images:
        for image in args.images:
            command_args.extend(["--image", image])
    if args.thread_id:
        command_args.extend(["resume", args.thread_id])

    return command_args


def _build_env(
    env_override: dict[str, str] | None, base_url: str | None, api_key: str | None
) -> dict[str, str]:
    if env_override is not None:
        env = dict(env_override)
    else:
        env = {k: v for k, v in os.environ.items() if v is not None}

    if INTERNAL_ORIGINATOR_ENV not in env:
        env[INTERNAL_ORIGINATOR_ENV] = PYTHON_SDK_ORIGINATOR
    if base_url:
        env["OPENAI_BASE_URL"] = base_url
    if api_key:
        env["CODEX_API_KEY"] = api_key
    return env


def _ensure_auth(env: dict[str, str], api_key: str | None) -> None:
    if api_key:
        return
    if env.get("CODEX_API_KEY") or env.get("OPENAI_API_KEY"):
        return

    codex_home = Path(env.get("CODEX_HOME", "~/.codex")).expanduser()
    auth_file = codex_home / "auth.json"
    if auth_file.exists():
        return

    raise _auth_required_error()


def _codex_not_installed_error() -> CodexNotInstalledError:
    return CodexNotInstalledError(
        "Codex CLI not found. Install with `npm install -g @openai/codex` or "
        "`brew install --cask codex`, then run `codex` to sign in. "
        "See the Codex CLI README for full instructions."
    )


def _auth_required_error() -> AuthRequiredError:
    return AuthRequiredError(
        "Codex authentication required. Sign in with ChatGPT (Plus/Pro/Team/Edu/Enterprise) "
        "by running `codex` and choosing 'Sign in with ChatGPT', or provide an API key "
        "via `api_key` or CODEX_API_KEY. See the Codex CLI README for details."
    )


def _terminate_process(process: subprocess.Popen) -> None:
    try:
        if process.poll() is None:
            process.kill()
    except Exception:
        pass


def _is_aborted(signal: object | None) -> bool:
    if signal is None:
        return False
    if isinstance(signal, AbortSignal):
        return signal.aborted
    if hasattr(signal, "aborted"):
        return bool(getattr(signal, "aborted"))
    if hasattr(signal, "is_set"):
        try:
            return bool(signal.is_set())  # type: ignore[call-arg]
        except TypeError:
            return bool(signal.is_set)  # type: ignore[attr-defined]
    return False


def _abort_reason(signal: object | None) -> str:
    if signal is None:
        return "Aborted"
    if isinstance(signal, AbortSignal):
        return str(signal.reason) if signal.reason is not None else "Aborted"
    reason = getattr(signal, "reason", None)
    if reason is not None:
        return str(reason)
    return "Aborted"


def find_codex_path() -> str:
    env_override = os.getenv("CODEX_EXECUTABLE")
    if env_override:
        return env_override

    vendor_path = _vendor_codex_path()
    if vendor_path:
        return vendor_path

    resolved = shutil.which("codex")
    if resolved:
        return resolved

    raise _codex_not_installed_error()


def _vendor_codex_path() -> str | None:
    system = platform.system().lower()
    arch = platform.machine().lower()

    target_triple = None
    if system in {"linux", "android"}:
        if arch in {"x86_64", "amd64"}:
            target_triple = "x86_64-unknown-linux-musl"
        elif arch in {"aarch64", "arm64"}:
            target_triple = "aarch64-unknown-linux-musl"
    elif system == "darwin":
        if arch in {"x86_64", "amd64"}:
            target_triple = "x86_64-apple-darwin"
        elif arch in {"aarch64", "arm64"}:
            target_triple = "aarch64-apple-darwin"
    elif system == "windows":
        if arch in {"x86_64", "amd64"}:
            target_triple = "x86_64-pc-windows-msvc"
        elif arch in {"aarch64", "arm64"}:
            target_triple = "aarch64-pc-windows-msvc"

    if not target_triple:
        return None

    base_dir = Path(__file__).resolve().parent
    vendor_root = base_dir / "vendor" / target_triple / "codex"
    binary_name = "codex.exe" if system == "windows" else "codex"
    candidate = vendor_root / binary_name
    if candidate.exists():
        return str(candidate)
    return None
