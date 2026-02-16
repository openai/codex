#!/usr/bin/env python3
"""Very small bootstrap generator for future typed-model expansion.

Reads codex app-server protocol JSON schema and prints top-level schema keys.
This is intentionally minimal v0 utility so we can iterate toward full typed
codegen without blocking SDK usage.
"""

from __future__ import annotations

import json
from pathlib import Path


def main() -> None:
    repo_root = Path(__file__).resolve().parents[3]
    schema_path = repo_root / "codex-rs" / "app-server-protocol" / "schema" / "json" / "codex_app_server_protocol.schemas.json"
    data = json.loads(schema_path.read_text())
    defs = data.get("$defs", {})

    out = [
        "# Auto-generated seed (v0)\n",
        "# Top-level schema definitions discovered in Codex app-server protocol\n",
        f"count = {len(defs)}\n",
    ]
    for key in sorted(defs.keys())[:50]:
        out.append(f"- {key}\n")

    target = repo_root / "sdk" / "python" / "SCHEMA_SEED.md"
    target.write_text("".join(out))
    print(f"wrote {target}")


if __name__ == "__main__":
    main()
