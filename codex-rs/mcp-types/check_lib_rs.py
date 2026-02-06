#!/usr/bin/env python3

import subprocess
import sys
from pathlib import Path


def main() -> int:
    crate_dir = Path(__file__).resolve().parent
    generator = crate_dir / "generate_mcp_types.py"

    result = subprocess.run(
        [sys.executable, str(generator), "--check"],
        cwd=crate_dir,
        check=False,
    )
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
