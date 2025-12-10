from __future__ import annotations

from typing import Optional

from .exec import CodexExec
from .options import CodexOptions, ThreadOptions
from .thread import Thread


class Codex:
    """Main entry point for interacting with the Codex agent."""

    def __init__(self, options: Optional[CodexOptions] = None) -> None:
        opts = options or CodexOptions()
        self._options = opts
        self._exec = CodexExec(opts.codex_path_override, opts.env)

    def start_thread(self, options: Optional[ThreadOptions] = None) -> Thread:
        return Thread(self._exec, self._options, options or ThreadOptions(), None)

    def resume_thread(self, thread_id: str, options: Optional[ThreadOptions] = None) -> Thread:
        return Thread(self._exec, self._options, options or ThreadOptions(), thread_id)
