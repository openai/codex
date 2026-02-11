from __future__ import annotations

from pathlib import Path

import pytest

from openai_codex_sdk.output_schema_file import create_output_schema_file


@pytest.mark.asyncio
async def test_create_output_schema_file_writes_and_cleans() -> None:
    schema = {"type": "object", "properties": {"summary": {"type": "string"}}}
    schema_file = await create_output_schema_file(schema)

    assert schema_file.schema_path is not None
    path = Path(schema_file.schema_path)
    assert path.exists()

    await schema_file.cleanup()
    assert not path.exists()


@pytest.mark.asyncio
async def test_create_output_schema_file_rejects_non_object() -> None:
    with pytest.raises(ValueError, match="output_schema must be a plain JSON object"):
        await create_output_schema_file(["not", "an", "object"])
