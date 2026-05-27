from __future__ import annotations

import contextlib
import importlib.util
import sys
import tempfile
import zlib
from pathlib import Path
from typing import Any, Iterator

_SDK_PYTHON_DIR = Path(__file__).resolve().parents[1]
_SDK_PYTHON_STR = str(_SDK_PYTHON_DIR)
if _SDK_PYTHON_STR not in sys.path:
    sys.path.insert(0, _SDK_PYTHON_STR)

from _runtime_setup import ensure_runtime_package_installed


def _ensure_runtime_dependencies(sdk_python_dir: Path) -> None:
    if importlib.util.find_spec("pydantic") is not None:
        return

    python = sys.executable
    raise RuntimeError(
        "Missing required dependency: pydantic.\n"
        f"Interpreter: {python}\n"
        "Install dependencies with the same interpreter used to run this example:\n"
        f"  cd {sdk_python_dir} && uv sync\n"
        "Then activate `.venv`, or reinstall with the Python interpreter above."
    )


def ensure_local_sdk_src() -> Path:
    """Add sdk/python/src to sys.path so examples run without installing the package."""
    sdk_python_dir = _SDK_PYTHON_DIR
    src_dir = sdk_python_dir / "src"
    package_dir = src_dir / "openai_codex"
    if not package_dir.exists():
        raise RuntimeError(f"Could not locate local SDK package at {package_dir}")

    _ensure_runtime_dependencies(sdk_python_dir)

    src_str = str(src_dir)
    if src_str not in sys.path:
        sys.path.insert(0, src_str)
    return src_dir


def runtime_config():
    """Return an example-friendly AppServerConfig for repo-source SDK usage."""
    from openai_codex import AppServerConfig

    ensure_runtime_package_installed(sys.executable, _SDK_PYTHON_DIR)
    return AppServerConfig()


def _png_chunk(chunk_type: bytes, data: bytes) -> bytes:
    import struct

    payload = chunk_type + data
    checksum = zlib.crc32(payload) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + payload + struct.pack(">I", checksum)


def _generated_sample_png_bytes() -> bytes:
    import struct

    width = 96
    height = 96
    top_left = (120, 180, 255)
    top_right = (255, 220, 90)
    bottom_left = (90, 180, 95)
    bottom_right = (180, 85, 85)

    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            if y < height // 2 and x < width // 2:
                color = top_left
            elif y < height // 2:
                color = top_right
            elif x < width // 2:
                color = bottom_left
            else:
                color = bottom_right
            rows.extend(color)

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + _png_chunk(b"IHDR", header)
        + _png_chunk(b"IDAT", zlib.compress(bytes(rows)))
        + _png_chunk(b"IEND", b"")
    )


@contextlib.contextmanager
def temporary_sample_image_path() -> Iterator[Path]:
    with tempfile.TemporaryDirectory(prefix="codex-python-example-image-") as temp_root:
        image_path = Path(temp_root) / "generated_sample.png"
        image_path.write_bytes(_generated_sample_png_bytes())
        yield image_path


def server_label(metadata: Any) -> str:
    return f"{metadata.serverInfo.name} {metadata.serverInfo.version}"
