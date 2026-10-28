"""Check the contents of npm archives staged from canonical release packages."""

import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "codex-cli" / "scripts"))

from build_npm_package import CODEX_PLATFORM_PACKAGES


class NpmPackageTest(unittest.TestCase):
    def test_npm_archives_preserve_cli_runtime_without_standalone_executor(
        self,
    ) -> None:
        # Use an installed executable as prebuilt input: packaging must preserve its
        # bytes even when staging a package for another operating system.
        executable = shutil.which("whoami") or sys.executable
        for package, platform in CODEX_PLATFORM_PACKAGES.items():
            with self.subTest(package=package), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                target = platform["target_triple"]
                source = root / "vendor" / target
                suffix = ".exe" if platform["os"] == "win32" else ""
                metadata = {
                    "layoutVersion": 1,
                    "variant": "codex",
                    "version": "1.2.3",
                    "target": target,
                    "entrypoint": f"bin/codex{suffix}",
                    "execServer": f"bin/codex-exec-server{suffix}",
                    "resourcesDir": "codex-resources",
                    "pathDir": "codex-path",
                }
                files = [
                    metadata["entrypoint"],
                    metadata["execServer"],
                    f"bin/codex-code-mode-host{suffix}",
                    f"codex-path/rg{suffix}",
                ]
                if platform["os"] == "linux":
                    files.append("codex-resources/bwrap")
                elif platform["os"] == "win32":
                    files.extend(
                        [
                            "codex-resources/codex-command-runner.exe",
                            "codex-resources/codex-windows-sandbox-setup.exe",
                        ]
                    )
                for name in files:
                    destination = source / name
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(executable, destination)
                (source / "codex-package.json").write_text(json.dumps(metadata))
                original = {
                    path.relative_to(source).as_posix(): path.read_bytes()
                    for path in source.rglob("*")
                    if path.is_file()
                }
                archive_path = root / "package.tgz"
                subprocess.run(
                    [
                        sys.executable,
                        str(
                            REPO_ROOT / "codex-cli" / "scripts" / "build_npm_package.py"
                        ),
                        "--package",
                        package,
                        "--version",
                        "1.2.3",
                        "--vendor-src",
                        str(source.parent),
                        "--staging-dir",
                        str(root / "staged"),
                        "--pack-output",
                        str(archive_path),
                    ],
                    check=True,
                    capture_output=True,
                )
                with tarfile.open(archive_path, "r:gz") as archive:
                    prefix = f"package/vendor/{target}/"
                    actual = {
                        member.name.removeprefix(prefix): archive.extractfile(
                            member
                        ).read()
                        for member in archive.getmembers()
                        if member.isfile() and member.name.startswith(prefix)
                    }
                expected = original.copy()
                metadata = json.loads(expected.pop("codex-package.json"))
                expected.pop(metadata.pop("execServer"))
                self.assertEqual(json.loads(actual.pop("codex-package.json")), metadata)
                self.assertEqual(actual, expected)
                self.assertEqual(
                    {
                        path.relative_to(source).as_posix(): path.read_bytes()
                        for path in source.rglob("*")
                        if path.is_file()
                    },
                    original,
                )


if __name__ == "__main__":
    unittest.main()
