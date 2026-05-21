#!/usr/bin/env python3

from contextlib import redirect_stdout
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest


INSTALL_SCRIPT = Path(__file__).resolve().parent / "install_native_deps.py"
SPEC = importlib.util.spec_from_file_location("install_native_deps", INSTALL_SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"Unable to load module from {INSTALL_SCRIPT}")

install_native_deps = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(install_native_deps)


class InstallCodexPackageArchivesTest(unittest.TestCase):
    def test_installs_codex_package_archive(self) -> None:
        target = "x86_64-unknown-linux-musl"
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            artifact_dir = root / "artifacts" / target
            package_src = root / "package-src"
            vendor_dir = root / "vendor"
            artifact_dir.mkdir(parents=True)
            (package_src / "bin").mkdir(parents=True)
            (package_src / "bin" / "codex").write_text("codex\n", encoding="utf-8")
            (package_src / "codex-package.json").write_text("{}\n", encoding="utf-8")

            archive_path = artifact_dir / f"codex-package-{target}.tar.gz"
            with tarfile.open(archive_path, "w:gz") as archive:
                archive.add(package_src / "bin", arcname="bin")
                archive.add(package_src / "codex-package.json", arcname="codex-package.json")

            with redirect_stdout(io.StringIO()):
                install_native_deps.install_codex_package_archives(
                    root / "artifacts",
                    vendor_dir,
                    [target],
                )

            self.assertEqual(
                sorted(
                    path.relative_to(vendor_dir / target)
                    for path in (vendor_dir / target).rglob("*")
                ),
                [
                    Path("bin"),
                    Path("bin/codex"),
                    Path("codex-package.json"),
                ],
            )

    def test_missing_codex_package_archive_errors(self) -> None:
        target = "x86_64-unknown-linux-musl"
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)

            with redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(
                    FileNotFoundError,
                    "Expected package archive not found",
                ):
                    install_native_deps.install_codex_package_archives(
                        root / "artifacts",
                        root / "vendor",
                        [target],
                    )


if __name__ == "__main__":
    unittest.main()
