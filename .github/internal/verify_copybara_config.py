#!/usr/bin/env python3

import os
import runpy
import shlex
import subprocess
import sys
import tempfile
import textwrap
from pathlib import Path

CONFIG_PATH = Path(".copybara/copy.bara.sky")
PUBLIC_IMPORTER = runpy.run_path(
    Path(__file__).with_name("copybara_public_to_internal.py").as_posix()
)
internal_path_for_public_path = PUBLIC_IMPORTER["internal_path_for_public_path"]


def main() -> int:
    copybara_jar = os.environ.get("COPYBARA_JAR")
    if not copybara_jar:
        print("COPYBARA_JAR must be set", file=sys.stderr)
        return 1

    with tempfile.TemporaryDirectory(prefix="copybara-config-check-") as temp_dir:
        temp_root = Path(temp_dir)
        copybara_home = temp_root / "home"
        output_root = temp_root / "output-root"
        copybara_home.mkdir()
        output_root.mkdir()

        base_copybara_args = [
            "java",
            f"-Duser.home={copybara_home}",
            "-jar",
            copybara_jar,
            f"--output-root={output_root}",
        ]

        validate_config(base_copybara_args)
        verify_marker_projection(base_copybara_args, temp_root)
        verify_public_import_projection(base_copybara_args, temp_root)

    return 0


def validate_config(base_copybara_args: list[str]) -> None:
    run([*base_copybara_args, "validate", CONFIG_PATH.as_posix()])


def verify_marker_projection(base_copybara_args: list[str], temp_root: Path) -> None:
    origin = temp_root / "marker-origin"
    destination = temp_root / "marker-destination"
    internal_crate = (
        origin / "codex-rs" / "internal-persistent-mode" / "src" / "lib.rs"
    )
    internal_crate.parent.mkdir(parents=True)
    internal_crate.write_text("internal persistent mode implementation\n", encoding="utf-8")

    cargo_manifest = origin / "codex-rs" / "Cargo.toml"
    cargo_manifest.parent.mkdir(parents=True, exist_ok=True)
    cargo_manifest.write_text(
        textwrap.dedent(
            """\
            [workspace.dependencies]
            # copybara:replace-for-public begin
            codex-otel = { package = "codex-internal-otel", path = "internal-otel" }
            # copybara:replace-for-public with
            # copybara:public codex-otel = { path = "otel" }
            # copybara:replace-for-public end
            other = "1"
            internal-only = "1" # copybara:strip-for-public

            # copybara:strip-for-public begin
            [workspace.metadata.codex-internal]
            enabled = true
            # copybara:strip-for-public end

            [dependencies]
            codex-otel = { workspace = true }
            """
        ),
        encoding="utf-8",
    )
    app_server_manifest = origin / "codex-rs" / "app-server" / "Cargo.toml"
    app_server_manifest.parent.mkdir(parents=True)
    app_server_manifest.write_text(
        textwrap.dedent(
            """\
            [dependencies]
            codex-internal-persistent-mode = { workspace = true } # copybara:strip-for-public
            other = { workspace = true }
            """
        ),
        encoding="utf-8",
    )
    extensions = origin / "codex-rs" / "app-server" / "src" / "extensions.rs"
    extensions.parent.mkdir(parents=True)
    extensions.write_text(
        textwrap.dedent(
            """\
            fn install() {
                // copybara:strip-for-public begin
                codex_internal_persistent_mode::install();
                // copybara:strip-for-public end
                install_public_extensions();
            }
            """
        ),
        encoding="utf-8",
    )
    thread_store_types = origin / "codex-rs" / "thread-store" / "src" / "types.rs"
    thread_store_types.parent.mkdir(parents=True)
    thread_store_types.write_text(
        textwrap.dedent(
            """\
            // copybara:replace-for-public begin
            pub struct PersistentModeConfig {
                pub message: String,
            }
            // copybara:replace-for-public with
            // copybara:public pub struct PersistentModeConfig {}
            // copybara:replace-for-public end
            """
        ),
        encoding="utf-8",
    )

    run(
        [
            *base_copybara_args,
            "migrate",
            CONFIG_PATH.as_posix(),
            "internal_to_public_projection",
            origin.as_posix(),
            f"--folder-dir={destination}",
            "--force-author=Copybara Config Check <copybara-config-check@openai.com>",
            "--force-message=Verify Copybara marker projection",
        ]
    )

    projected_manifest = destination / "codex-rs" / "Cargo.toml"
    actual = projected_manifest.read_text(encoding="utf-8")
    assert_contains(projected_manifest, actual, 'codex-otel = { path = "otel" }')
    assert_contains(projected_manifest, actual, 'codex-otel = { workspace = true }')
    assert_not_contains(projected_manifest, actual, "codex-internal-otel")
    assert_not_contains(projected_manifest, actual, "internal-only")
    assert_not_contains(projected_manifest, actual, "workspace.metadata.codex-internal")
    assert_not_contains(projected_manifest, actual, "copybara:")
    assert_not_exists(destination / "codex-rs" / "internal-persistent-mode")

    projected_app_server_manifest = (
        destination / "codex-rs" / "app-server" / "Cargo.toml"
    )
    actual = projected_app_server_manifest.read_text(encoding="utf-8")
    assert_contains(projected_app_server_manifest, actual, "other = { workspace = true }")
    assert_not_contains(
        projected_app_server_manifest, actual, "codex-internal-persistent-mode"
    )
    assert_not_contains(projected_app_server_manifest, actual, "copybara:")

    projected_extensions = (
        destination / "codex-rs" / "app-server" / "src" / "extensions.rs"
    )
    actual = projected_extensions.read_text(encoding="utf-8")
    assert_contains(projected_extensions, actual, "install_public_extensions();")
    assert_not_contains(projected_extensions, actual, "codex_internal_persistent_mode")
    assert_not_contains(projected_extensions, actual, "copybara:")

    projected_thread_store_types = (
        destination / "codex-rs" / "thread-store" / "src" / "types.rs"
    )
    actual = projected_thread_store_types.read_text(encoding="utf-8")
    assert_contains(projected_thread_store_types, actual, "pub struct PersistentModeConfig")
    assert_not_contains(projected_thread_store_types, actual, "pub message")
    assert_not_contains(projected_thread_store_types, actual, "copybara:")


def verify_public_import_projection(
    base_copybara_args: list[str], temp_root: Path
) -> None:
    verify_public_import_projection_case(
        base_copybara_args,
        temp_root,
        case_name="with-public-directory",
        include_public_directory=True,
    )
    verify_public_import_projection_case(
        base_copybara_args,
        temp_root,
        case_name="without-public-directory",
        include_public_directory=False,
    )


def verify_public_import_projection_case(
    base_copybara_args: list[str],
    temp_root: Path,
    *,
    case_name: str,
    include_public_directory: bool,
) -> None:
    origin = temp_root / f"public-import-origin-{case_name}"
    destination = temp_root / f"public-import-destination-{case_name}"
    public_files = {
        Path("LICENSE"): "public license\n",
        Path("MODULE.bazel"): 'module(name = "codex")\n',
        Path("MODULE.bazel.lock"): "{}\n",
        Path("codex-cli/bin/codex.js"): "console.log('codex');\n",
        Path("codex-rs/core/src/lib.rs"): "pub fn public_api() {}\n",
        Path("sdk/python/README.md"): "sdk readme\n",
        Path("third_party/example/README.md"): "third party readme\n",
        Path("README.md"): "public readme\n",
        Path(".github/workflows/ci.yml"): "name: Public CI\n",
        Path(".vscode/settings.json"): "{}\n",
        Path("docs/example.md"): "public documentation\n",
    }
    if include_public_directory:
        public_files[Path("public/nested.txt")] = "nested public directory\n"
    for path, contents in public_files.items():
        source = origin / path
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text(contents, encoding="utf-8")

    preserved_internal_files = {
        Path("README.md"): "internal readme\n",
        Path(".github/internal/tool.py"): "internal automation\n",
        Path("codex-rs/internal-example/src/lib.rs"): "internal crate\n",
    }
    for path, contents in preserved_internal_files.items():
        internal_file = destination / path
        internal_file.parent.mkdir(parents=True, exist_ok=True)
        internal_file.write_text(contents, encoding="utf-8")
    stale_public_file = destination / "public/stale.md"
    stale_public_file.parent.mkdir(parents=True, exist_ok=True)
    stale_public_file.write_text("stale public file\n", encoding="utf-8")

    run(
        [
            *base_copybara_args,
            "migrate",
            CONFIG_PATH.as_posix(),
            "public_to_internal_projection",
            origin.as_posix(),
            f"--folder-dir={destination}",
            "--force-author=Copybara Config Check <copybara-config-check@openai.com>",
            "--force-message=Verify public import projection",
        ]
    )

    for public_path, expected_contents in public_files.items():
        internal_path = internal_path_for_public_path(public_path)
        projected = destination / internal_path
        actual = projected.read_text(encoding="utf-8")
        if actual != expected_contents:
            raise RuntimeError(
                f"{public_path} projected to {internal_path} with unexpected contents"
            )
        if internal_path != public_path and public_path not in preserved_internal_files:
            assert_not_exists(destination / public_path)

    for path, expected_contents in preserved_internal_files.items():
        actual = (destination / path).read_text(encoding="utf-8")
        if actual != expected_contents:
            raise RuntimeError(
                f"Public import unexpectedly changed internal path {path}"
            )
    assert_not_exists(stale_public_file)


def assert_contains(path: Path, text: str, expected: str) -> None:
    if expected not in text:
        raise RuntimeError(f"{path} did not contain expected text: {expected}")


def assert_not_contains(path: Path, text: str, unexpected: str) -> None:
    if unexpected in text:
        raise RuntimeError(f"{path} contained unexpected text: {unexpected}")


def assert_not_exists(path: Path) -> None:
    if path.exists():
        raise RuntimeError(f"{path} should not exist")


def run(args: list[str]) -> None:
    print(f"+ {shlex.join(args)}", flush=True)
    subprocess.run(args, check=True)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1)
