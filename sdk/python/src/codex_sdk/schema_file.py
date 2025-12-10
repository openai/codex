from __future__ import annotations

import json
import shutil
import tempfile
from pathlib import Path
from typing import Callable, Optional, Tuple


SchemaFile = Tuple[Optional[str], Callable[[], None]]


def create_output_schema_file(schema: object) -> SchemaFile:
    if schema is None:
        return None, _noop_cleanup

    if not isinstance(schema, dict):
        raise ValueError("output_schema must be a plain JSON object")

    schema_dir = Path(tempfile.mkdtemp(prefix="codex-output-schema-"))
    schema_path = schema_dir / "schema.json"

    def cleanup() -> None:
        try:
            shutil.rmtree(schema_dir, ignore_errors=True)
        except Exception:
            pass

    try:
        schema_path.write_text(json.dumps(schema), encoding="utf-8")
        return str(schema_path), cleanup
    except Exception:
        cleanup()
        raise


def _noop_cleanup() -> None:
    return None
