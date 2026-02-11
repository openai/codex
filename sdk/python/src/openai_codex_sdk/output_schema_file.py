from __future__ import annotations

import json
import shutil
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Awaitable, Callable


@dataclass(slots=True)
class OutputSchemaFile:
    schema_path: str | None
    cleanup: Callable[[], Awaitable[None]]


async def create_output_schema_file(schema: Any) -> OutputSchemaFile:
    if schema is None:
        async def noop() -> None:
            return None

        return OutputSchemaFile(schema_path=None, cleanup=noop)

    if not isinstance(schema, dict):
        raise ValueError("output_schema must be a plain JSON object")

    schema_dir = Path(tempfile.mkdtemp(prefix="codex-output-schema-"))
    schema_path = schema_dir / "schema.json"

    async def cleanup() -> None:
        shutil.rmtree(schema_dir, ignore_errors=True)

    try:
        schema_path.write_text(json.dumps(schema), encoding="utf-8")
        return OutputSchemaFile(schema_path=str(schema_path), cleanup=cleanup)
    except Exception:
        await cleanup()
        raise
