#!/usr/bin/env python3
from __future__ import annotations

from pydantic import BaseModel

from codex_sdk import Codex
from helpers import codex_path_override


class SummarySchema(BaseModel):
    summary: str
    status: str


codex = Codex(codex_path_override=codex_path_override())
thread = codex.start_thread()

turn = thread.run("Summarize repository status", output_schema=SummarySchema)
print(turn.final_response)
