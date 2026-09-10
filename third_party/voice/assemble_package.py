"""Add a helper and its prepared runtime to a fresh private Codex package."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys

# Import only this script's siblings, including under PYTHONSAFEPATH.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from package_runtime import runtime_files
from runtime import digest


def assemble(
    package: Path,
    helper: Path,
    voice_target: str,
    commit: str,
    output: Path,
    *,
    runtime: Path,
    release_version: str | None = None,
):
    package, helper = package.resolve(strict=True), helper.resolve(strict=True)
    output = output.absolute()
    if (
        output.exists()
        or output.is_symlink()
        or output.resolve().is_relative_to(package)
    ):
        raise ValueError("output must be fresh and outside the input package")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError(
            "a full build commit is required; dev builds are not distributable"
        )
    metadata = json.loads((package / "codex-package.json").read_text())
    app_target = metadata["target"]
    targets = {
        f"{arch}-{suffix}": f"{arch}-{suffix.replace('musl', 'gnu')}"
        for arch in ("aarch64", "x86_64")
        for suffix in (
            "apple-darwin",
            "unknown-linux-gnu",
            "unknown-linux-musl",
            "pc-windows-msvc",
        )
    }
    if targets.get(app_target) != voice_target:
        raise ValueError("incompatible app and helper targets")
    suffix = ".exe" if app_target.endswith("windows-msvc") else ""
    entrypoint = f"bin/codex{suffix}"
    expected = {
        "layoutVersion": 1,
        "variant": "codex",
        "entrypoint": entrypoint,
        "resourcesDir": "codex-resources",
        "pathDir": "codex-path",
    }
    if any(metadata.get(key) != value for key, value in expected.items()):
        raise ValueError("input is not a canonical Codex package")
    if release_version is None:
        if not metadata["version"].endswith(f"+{commit}"):
            raise ValueError("package version does not match the declared build")
    elif (
        not re.fullmatch(
            r"[0-9]+\.[0-9]+\.[0-9]+(?:-alpha(?:\.[0-9]+){0,2}|-beta(?:\.[0-9]+)?)?",
            release_version,
        )
        or metadata["version"] != release_version
    ):
        raise ValueError("package version does not match the release")
    if (package / "codex-resources/voice").exists():
        raise ValueError("input already contains voice resources")
    for path in package.rglob("*"):
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise ValueError("package inputs must be regular files or directories")
    if not helper.is_file() or not (package / entrypoint).is_file():
        raise ValueError("helper and app entrypoint must be regular files")
    if not suffix and not helper.stat().st_mode & 0o111:
        raise ValueError("helper is not executable")
    runtime = runtime.resolve(strict=True)
    if any(parent.samefile(package) for parent in (runtime, *runtime.parents)):
        raise ValueError("runtime must be outside the input package")
    if any(
        parent.exists() and parent.samefile(runtime)
        for parent in output.resolve().parents
    ):
        raise ValueError("output must be outside the runtime input")
    inputs = runtime_files(
        runtime, voice_target, public_release=release_version is not None
    )
    if release_version is not None:
        receipt = json.loads((runtime / "runtime.json").read_text(encoding="utf-8"))
        if receipt["sourceCommit"] != commit:
            raise ValueError("release runtime source does not match the app build")
    output.mkdir()  # Exclusive creation: never clean or overwrite a pre-existing output.
    try:
        shutil.copytree(package, output, dirs_exist_ok=True)
        relative_helper = f"codex-resources/voice/bin/codex-voice-host{suffix}"
        destination = output / relative_helper
        destination.parent.mkdir(parents=True)
        shutil.copy2(helper, destination)
        for relative, expected_digest in inputs.items():
            copied = output / "codex-resources/voice" / relative
            copied.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(runtime / relative, copied)
            if digest(copied) != expected_digest:
                raise ValueError("runtime file changed during copying")
        if release_version is not None:
            source_dir = Path(__file__).resolve().parent
            for relative in (
                "NOTICE.md",
                "sources.json",
                "licenses/LGPL-2.1.txt",
                "licenses/Opus.txt",
                "licenses/PCRE2.md",
                "licenses/libffi.txt",
                "licenses/proxy-libintl.txt",
                "licenses/sljit.txt",
                "licenses/zlib.txt",
            ):
                destination_file = destination.parent.parent / relative
                destination_file.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source_dir / relative, destination_file)
                inputs[relative] = digest(destination_file)
        digests = {}
        for relative in (entrypoint, relative_helper):
            with (output / relative).open("rb") as source:
                digests[relative] = hashlib.file_digest(source, "sha256").hexdigest()
        digests.update(
            {f"codex-resources/voice/{name}": value for name, value in inputs.items()}
        )
        manifest = {
            "schemaVersion": 1,
            "buildCommit": commit,
            "appTarget": app_target,
            "voiceTarget": voice_target,
            "appVersion": metadata["version"],
            "sha256": digests,
        }
        (destination.parent.parent / "manifest.json").write_text(
            json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
        )
    except BaseException:
        shutil.rmtree(output)
        raise


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--helper", type=Path, required=True)
    parser.add_argument("--voice-target", required=True)
    parser.add_argument("--build-commit", required=True)
    parser.add_argument(
        "--release-version", help="exact version of a public release package"
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--runtime",
        type=Path,
        required=True,
        help="prepared runtime required by the helper",
    )
    args = parser.parse_args()
    assemble(
        args.package,
        args.helper,
        args.voice_target,
        args.build_commit,
        args.output,
        runtime=args.runtime,
        release_version=args.release_version,
    )
