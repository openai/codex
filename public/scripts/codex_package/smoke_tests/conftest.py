from pathlib import Path

import pytest
from fixtures import code_mode_host_debug_symbols, package, responses_server

__all__ = ["code_mode_host_debug_symbols", "package", "responses_server"]


def pytest_addoption(parser: pytest.Parser) -> None:
    group = parser.getgroup("Codex package smoke")
    group.addoption(
        "--compression",
        choices=("gzip", "zstd"),
        required=True,
        help="Compression format of the supplied package archives.",
    )
    group.addoption(
        "--package-target",
        required=True,
        help="Rust target triple for the assembled release packages.",
    )
    group.addoption(
        "--cli-archive",
        type=Path,
        required=True,
        help="Assembled Codex CLI package archive in the selected format.",
    )
    group.addoption(
        "--app-server-archive",
        type=Path,
        required=True,
        help="Assembled app-server package archive in the selected format.",
    )
    group.addoption(
        "--symbols-archive",
        type=Path,
        required=True,
        help="Gzip archive containing symbols for all packaged binaries.",
    )
