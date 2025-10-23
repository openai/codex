from __future__ import annotations

from typing import Optional

from .config import CodexOptions, ThreadOptions
from .exec import CodexExec
from .thread import Thread


class Codex:
    def __init__(self, options: Optional[CodexOptions] = None) -> None:
        opts = options or CodexOptions()
        self._options = opts
        self._exec = CodexExec(opts.codex_path_override)

    def start_thread(self, options: Optional[ThreadOptions] = None) -> Thread:
        thread_options = options or ThreadOptions()
        return Thread(self._exec, self._options, thread_options)

    def resume_thread(self, thread_id: str, options: Optional[ThreadOptions] = None) -> Thread:
        thread_options = options or ThreadOptions()
        return Thread(self._exec, self._options, thread_options, thread_id)
