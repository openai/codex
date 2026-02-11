from __future__ import annotations

from typing import Any, Mapping

from .exec import CodexExec
from .options import (
    CodexOptions,
    ThreadOptions,
    coerce_codex_options,
    coerce_thread_options,
)
from .thread import Thread


class Codex:
    """
    Main entrypoint for interacting with the Codex agent.

    Use start_thread() for a new conversation, or resume_thread() with an
    existing thread id persisted by Codex.
    """

    def __init__(self, options: CodexOptions | Mapping[str, Any] | None = None) -> None:
        self._options = coerce_codex_options(options)
        self._exec = CodexExec(
            executable_path=self._options.codex_path_override,
            env=self._options.env,
            config_overrides=self._options.config,
        )

    def start_thread(self, options: ThreadOptions | Mapping[str, Any] | None = None) -> Thread:
        return Thread(
            _exec=self._exec,
            _options=self._options,
            _thread_options=coerce_thread_options(options),
        )

    def resume_thread(
        self, thread_id: str, options: ThreadOptions | Mapping[str, Any] | None = None
    ) -> Thread:
        return Thread(
            _exec=self._exec,
            _options=self._options,
            _thread_options=coerce_thread_options(options),
            _id=thread_id,
        )
