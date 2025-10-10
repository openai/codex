from __future__ import annotations

import json
import tempfile
from collections.abc import Mapping
from pathlib import Path
from types import TracebackType
from typing import Any, Type, cast
from functools import lru_cache

from .exceptions import SchemaValidationError
from .config import SchemaInput


@lru_cache(maxsize=1)
def _get_pydantic_base_model() -> Type[Any] | None:  # pragma: no cover - import guard
    try:
        from pydantic import BaseModel
    except ImportError:
        return None
    return cast(Type[Any], BaseModel)


def _is_pydantic_model(value: object) -> bool:
    base_model = _get_pydantic_base_model()
    return isinstance(value, type) and base_model is not None and issubclass(value, base_model)


def _is_pydantic_instance(value: object) -> bool:
    base_model = _get_pydantic_base_model()
    return base_model is not None and isinstance(value, base_model)


def _convert_schema_input(schema: SchemaInput | None) -> Mapping[str, object] | None:
    if schema is None or isinstance(schema, Mapping):
        return schema

    if _is_pydantic_model(schema):
        return cast(Mapping[str, object], schema.model_json_schema())

    if _is_pydantic_instance(schema):
        return cast(Mapping[str, object], schema.model_json_schema())

    raise SchemaValidationError(
        "output_schema must be a mapping or a Pydantic BaseModel (class or instance)",
    )


class SchemaTempFile:
    def __init__(self, schema: SchemaInput | None) -> None:
        self._raw_schema = schema
        self._temp_dir: tempfile.TemporaryDirectory[str] | None = None
        self.path: Path | None = None

    def __enter__(self) -> SchemaTempFile:
        schema = _convert_schema_input(self._raw_schema)
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


def prepare_schema_file(schema: SchemaInput | None) -> SchemaTempFile:
    return SchemaTempFile(schema)
