from __future__ import annotations

import io
import os
import subprocess
from dataclasses import dataclass
from threading import Thread
from typing import Iterator, Optional

from .config import SandboxMode
from .discovery import find_codex_binary
from .exceptions import ExecExitError, SpawnError

INTERNAL_ORIGINATOR_ENV = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE"
PYTHON_SDK_ORIGINATOR = "codex_sdk_py"


@dataclass(frozen=True, slots=True)
class ExecArgs:
    input: str
    base_url: Optional[str] = None
    api_key: Optional[str] = None
    thread_id: Optional[str] = None
    model: Optional[str] = None
    sandbox_mode: Optional[SandboxMode] = None
    working_directory: Optional[str] = None
    skip_git_repo_check: bool = False
    output_schema_path: Optional[str] = None


class CodexExec:
    def __init__(self, executable_override: Optional[str] = None) -> None:
        self._binary = find_codex_binary(executable_override)

    def build_command(self, args: ExecArgs) -> list[str]:
        command = [str(self._binary), "exec", "--experimental-json"]

        if args.model:
            command.extend(["--model", args.model])
        if args.sandbox_mode:
            command.extend(["--sandbox", args.sandbox_mode.value])
        if args.working_directory:
            command.extend(["--cd", args.working_directory])
        if args.skip_git_repo_check:
            command.append("--skip-git-repo-check")
        if args.output_schema_path:
            command.extend(["--output-schema", args.output_schema_path])
        if args.thread_id:
            command.extend(["resume", args.thread_id])

        return command

    def run_lines(self, args: ExecArgs) -> Iterator[str]:
        command = self.build_command(args)

        env = os.environ.copy()
        env.setdefault(INTERNAL_ORIGINATOR_ENV, PYTHON_SDK_ORIGINATOR)
        if args.base_url:
            env["OPENAI_BASE_URL"] = args.base_url
        if args.api_key:
            env["CODEX_API_KEY"] = args.api_key

        stderr_buffer: list[str] = []

        try:
            process = subprocess.Popen(
                command,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding="utf-8",
                errors="strict",
                env=env,
            )
        except OSError as error:  # pragma: no cover - exercised indirectly
            raise SpawnError(command, error) from error

        if not process.stdin or not process.stdout:
            process.kill()
            raise SpawnError(command, OSError("Missing stdio pipes"))

        stderr_thread: Thread | None = None
        if process.stderr:
            def _drain_stderr(pipe: io.TextIOBase, buffer: list[str]) -> None:
                while True:
                    try:
                        chunk = pipe.readline()
                    except ValueError:
                        break
                    if chunk == "":
                        break
                    buffer.append(chunk)

            stderr_thread = Thread(
                target=_drain_stderr,
                args=(process.stderr, stderr_buffer),
                daemon=True,
            )
            stderr_thread.start()

        try:
            process.stdin.write(args.input)
            process.stdin.close()

            for line in iter(process.stdout.readline, ""):
                yield line.rstrip("\n")

            return_code = process.wait()
            if stderr_thread is not None:
                stderr_thread.join()

            stderr_output = "".join(stderr_buffer)
            if return_code != 0:
                raise ExecExitError(tuple(command), return_code, stderr_output)
        finally:
            if process.stdout and not process.stdout.closed:
                process.stdout.close()
            if process.stderr and not process.stderr.closed:
                try:
                    process.stderr.close()
                except ValueError:
                    pass
            if stderr_thread is not None and stderr_thread.is_alive():
                stderr_thread.join(timeout=0.1)
            returncode = process.poll()
            if returncode is None:
                process.kill()
                try:
                    process.wait(timeout=0.5)
                except subprocess.TimeoutExpired:
                    process.wait()
