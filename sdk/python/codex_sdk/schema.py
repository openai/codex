from __future__ import annotations

import json
import os
import shutil
import tempfile
from dataclasses import dataclass
from typing import Any, Mapping

from pydantic import BaseModel


@dataclass
class OutputSchemaFile:
    schema_path: str | None
    cleanup: callable


def _is_json_object(value: object) -> bool:
    return isinstance(value, Mapping) and not isinstance(value, list)


def _convert_pydantic_schema(schema: object) -> dict[str, Any] | None:
    if isinstance(schema, BaseModel):
        return schema.model_json_schema()
    if isinstance(schema, type) and issubclass(schema, BaseModel):
        return schema.model_json_schema()
    if hasattr(schema, "model_json_schema"):
        try:
            return schema.model_json_schema()  # type: ignore[call-arg]
        except TypeError:
            return None
    return None


def normalize_output_schema(schema: object | None) -> dict[str, Any] | None:
    if schema is None:
        return None

    converted = _convert_pydantic_schema(schema)
    if converted is not None:
        return converted

    if not _is_json_object(schema):
        raise ValueError("output_schema must be a plain JSON object or Pydantic model")

    return dict(schema)  # shallow copy


def create_output_schema_file(schema: object | None) -> OutputSchemaFile:
    normalized = normalize_output_schema(schema)
    if normalized is None:
        return OutputSchemaFile(schema_path=None, cleanup=lambda: None)

    schema_dir = tempfile.mkdtemp(prefix="codex-output-schema-")
    schema_path = os.path.join(schema_dir, "schema.json")

    def cleanup() -> None:
        try:
            shutil.rmtree(schema_dir, ignore_errors=True)
        except Exception:
            pass

    try:
        with open(schema_path, "w", encoding="utf-8") as handle:
            json.dump(normalized, handle)
        return OutputSchemaFile(schema_path=schema_path, cleanup=cleanup)
    except Exception:
        cleanup()
        raise
