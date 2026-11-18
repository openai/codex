#!/usr/bin/env python3

import argparse
import os
import subprocess
import sys
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Regenerate vendored app-server schema fixtures"
    )
    parser.add_argument(
        "--schema-root",
        type=Path,
        help="root directory containing the schema fixtures",
    )
    parser.add_argument(
        "-p",
        "--prettier",
        type=Path,
        help="optional Prettier executable used to format TypeScript files",
    )
    parser.add_argument(
        "--experimental",
        action="store_true",
        help="regenerate the precomputed experimental exports",
    )
    args = parser.parse_args()

    workspace_root = Path(__file__).resolve().parents[2]
    repository_schema_root = workspace_root / "app-server-protocol" / "schema"
    schema_root = args.schema_root or repository_schema_root

    env = os.environ.copy()
    env["CODEX_APP_SERVER_SCHEMA_ROOT"] = str(schema_root)
    env["CODEX_APP_SERVER_SCHEMA_EXPERIMENTAL"] = "1" if args.experimental else "0"
    if args.prettier:
        env["CODEX_APP_SERVER_SCHEMA_PRETTIER"] = str(args.prettier)

    subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "codex-app-server-protocol",
            "--lib",
            "schema_fixtures_tests::write_schema_fixtures_from_env",
            "--",
            "--exact",
            "--ignored",
        ],
        cwd=workspace_root,
        env=env,
        check=True,
    )

    # Scratch exports and experimental-only bundles do not update checked-in SDK code.
    if (
        not args.experimental
        and schema_root.resolve() == repository_schema_root.resolve()
    ):
        sdk_root = workspace_root.parent / "sdk" / "python"
        subprocess.run(
            [
                sys.executable,
                str(workspace_root / "scripts" / "run_locked_python_tool.py"),
                "--project",
                str(sdk_root),
                "--only-group",
                "test",
                "python",
                str(sdk_root / "scripts" / "update_sdk_artifacts.py"),
                "generate-types",
                "--schema-dir",
                str(schema_root / "json"),
            ],
            cwd=workspace_root,
            check=True,
        )


if __name__ == "__main__":
    main()
