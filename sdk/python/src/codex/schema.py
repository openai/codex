from __future__ import annotations

import json
import tempfile
from collections.abc import Mapping
from pathlib import Path
from types import TracebackType

from .exceptions import SchemaValidationError


class SchemaTempFile:
    def __init__(self, schema: Mapping[str, object] | None) -> None:
        self._schema = schema
        self._temp_dir: tempfile.TemporaryDirectory[str] | None = None
        self.path: Path | None = None

    def __enter__(self) -> SchemaTempFile:
        schema = self._schema
        if schema is None:
            return self

        for key in schema.keys():
            if not isinstance(key, str):
                raise SchemaValidationError("output_schema keys must be strings")

        self._temp_dir = tempfile.TemporaryDirectory(prefix="codex-output-schema-")
        schema_dir = Path(self._temp_dir.name)
        schema_path = schema_dir / "schema.json"

        with schema_path.open("w", encoding="utf-8") as handle:
            json.dump(schema, handle, ensure_ascii=False)
        self.path = schema_path
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        self.cleanup()

    def cleanup(self) -> None:
        if self._temp_dir is not None:
            self._temp_dir.cleanup()
            self._temp_dir = None
        self.path = None


def prepare_schema_file(schema: Mapping[str, object] | None) -> SchemaTempFile:
    if schema is not None and not isinstance(schema, Mapping):
        raise SchemaValidationError("output_schema must be a mapping")
    return SchemaTempFile(schema)
