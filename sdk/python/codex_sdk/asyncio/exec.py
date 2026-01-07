from __future__ import annotations

import asyncio
from typing import AsyncIterator

from ..errors import AbortError, CodexNotInstalledError, ThreadRunError
from ..exec import (
    CodexExecArgs,
    _abort_reason,
    _build_command_args,
    _build_env,
    _ensure_auth,
    _is_aborted,
    find_codex_path,
)


class AsyncCodexExec:
    def __init__(self, executable_path: str | None = None, env: dict[str, str] | None = None) -> None:
        self._executable_path = executable_path or find_codex_path()
        self._env_override = env

    async def run(self, args: CodexExecArgs) -> AsyncIterator[str]:
        if _is_aborted(args.signal):
            raise AbortError(_abort_reason(args.signal))

        command_args = _build_command_args(args)
        env = _build_env(self._env_override, args.base_url, args.api_key)
        _ensure_auth(env, args.api_key)

        try:
            process = await asyncio.create_subprocess_exec(
                self._executable_path,
                *command_args,
                stdin=asyncio.subprocess.PIPE,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                env=env,
            )
        except FileNotFoundError as exc:
            raise CodexNotInstalledError(
                "Codex CLI not found. Install with `npm install -g @openai/codex` or "
                "`brew install --cask codex`, then run `codex` to sign in. "
                "See the Codex CLI README for full instructions."
            ) from exc

        if process.stdin is None or process.stdout is None or process.stderr is None:
            _terminate_async_process(process)
            raise ThreadRunError("Child process missing standard streams")

        process.stdin.write(args.input.encode("utf-8"))
        await process.stdin.drain()
        process.stdin.close()

        stderr_task = asyncio.create_task(process.stderr.read())

        try:
            while True:
                if _is_aborted(args.signal):
                    _terminate_async_process(process)
                    raise AbortError(_abort_reason(args.signal))

                line = await process.stdout.readline()
                if not line:
                    break
                text_line = line.decode("utf-8").rstrip("\n")
                if text_line:
                    yield text_line

            returncode = await process.wait()
            stderr_bytes = await stderr_task
            if returncode != 0:
                stderr_text = stderr_bytes.decode("utf-8") if stderr_bytes else ""
                raise ThreadRunError(
                    f"Codex Exec exited with code {returncode}: {stderr_text}"
                )
        finally:
            _terminate_async_process(process)


def _terminate_async_process(process: asyncio.subprocess.Process) -> None:
    try:
        if process.returncode is None:
            process.kill()
    except Exception:
        pass
