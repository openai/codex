from __future__ import annotations

from .exec import CodexExec
from .options import CodexOptions, ThreadOptions
from .thread import Thread


class Codex:
    def __init__(self, options: CodexOptions | None = None, **kwargs: object) -> None:
        if options is not None and kwargs:
            raise ValueError("Provide either CodexOptions or keyword arguments, not both")
        if options is None:
            options = CodexOptions(**kwargs)
        self._options = options
        self._exec = CodexExec(options.codex_path_override, options.env)

    def start_thread(self, options: ThreadOptions | None = None, **kwargs: object) -> Thread:
        if options is not None and kwargs:
            raise ValueError("Provide either ThreadOptions or keyword arguments, not both")
        thread_options = options or ThreadOptions(**kwargs)
        return Thread(self._exec, self._options, thread_options)

    def resume_thread(
        self, thread_id: str, options: ThreadOptions | None = None, **kwargs: object
    ) -> Thread:
        if options is not None and kwargs:
            raise ValueError("Provide either ThreadOptions or keyword arguments, not both")
        thread_options = options or ThreadOptions(**kwargs)
        return Thread(self._exec, self._options, thread_options, thread_id)
