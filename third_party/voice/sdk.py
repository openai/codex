"""Export development inputs from the same inspected build as the private runtime.

SDKs are build inputs, never installed user payloads. Their receipt records byte
identity and provenance, not authenticity or approval. Native loader relocation
and final helper linking remain the runtime preparer and consumer's jobs.
"""

import json
from pathlib import Path
import re
import shutil

from runtime import digest

MODULES = (
    "glib-2.0",
    "gobject-2.0",
    "gmodule-no-export-2.0",
    "gstreamer-1.0",
    "gstreamer-base-1.0",
    "gstreamer-app-1.0",
    "gstreamer-audio-1.0",
    "gstreamer-tag-1.0",
)


def export_sdk(prefix: Path, receipts: Path, target: str, output: Path):
    prefix, receipts = prefix.resolve(strict=True), receipts.resolve(strict=True)
    output = output.absolute()
    if (
        output.exists()
        or output.is_symlink()
        or any(output.resolve().is_relative_to(root) for root in (prefix, receipts))
    ):
        raise ValueError("SDK output must be fresh and outside the inputs")
    if not re.fullmatch(
        r"(?:aarch64|x86_64)-(?:apple-darwin|unknown-linux-gnu|pc-windows-msvc)",
        target,
    ):
        raise ValueError("unsupported SDK target")
    ci = json.loads((receipts / "ci.json").read_text())
    source_hash = digest(Path(__file__).with_name("sources.json"))
    if (
        ci.get("target") != target
        or ci.get("build_complete") is not True
        or ci.get("inspection_complete") is not True
        or ci.get("manifest_sha256") != source_hash
        or not re.fullmatch(r"[0-9a-f]{40}", ci.get("commit", ""))
    ):
        raise ValueError("SDK build receipt does not match the sources and target")
    inventory = json.loads((receipts / "inspection/binaries.json").read_text())
    if not 1 <= len(inventory) <= 128:
        raise ValueError("unexpected native inventory size")
    binaries = {}
    for record in inventory:
        name = record["path"].replace("\\", "/")
        if not re.fullmatch(r"(?:lib(?:/gstreamer-1\.0)?|bin)/[A-Za-z0-9_+.-]+", name):
            raise ValueError("invalid native inventory path")
        path = prefix / name
        if (
            name in binaries
            or path.is_symlink()
            or not path.resolve(strict=True).is_relative_to(prefix)
            or record["target"] != target
            or digest(path) != record["sha256"]
        ):
            raise ValueError("native SDK library does not match its receipt")
        binaries[name] = record["sha256"]
    selected = []
    names = set()
    size = 0
    for path in sorted(prefix.rglob("*")):
        name = path.relative_to(prefix).as_posix()
        if path.is_dir():
            if path.is_symlink():
                raise ValueError("SDK input directories must not be links")
            continue
        if not (
            name.startswith("include/")
            or name == "lib/glib-2.0/include/glibconfig.h"
            or name
            in {
                f"lib/pkgconfig/{module}.pc"
                for module in (*MODULES, "libffi", "libpcre2-8", "zlib")
            }
            or re.fullmatch(
                r"lib/[A-Za-z0-9_+.-]+\.(?:a|lib|dylib|so(?:\.[0-9]+)*)", name
            )
        ):
            continue
        source = path.resolve(strict=True)
        if not source.is_relative_to(prefix) or not source.is_file():
            raise ValueError("SDK inputs must be files inside the prefix")
        if name.casefold() in names:
            raise ValueError("SDK paths collide across platforms")
        names.add(name.casefold())
        size += source.stat().st_size
        if len(selected) >= 4096 or size > 512 * 1024 * 1024:
            raise ValueError("SDK inputs exceed the size limit")
        source_name = source.relative_to(prefix).as_posix()
        expected = digest(source)
        if re.search(r"\.(?:dylib|so(?:\.[0-9]+)*)$", name):
            if binaries.get(source_name) != expected:
                raise ValueError("SDK shared library is missing from the receipt")
        selected.append((name, source, expected))
    for module in MODULES:
        path = prefix / f"lib/pkgconfig/{module}.pc"
        if path.relative_to(prefix).as_posix().casefold() not in names or not re.search(
            r"^prefix=\$\{pcfiledir\}/\.\./\.\.$", path.read_text(), re.MULTILINE
        ):
            raise ValueError("rebuild the SDK with relocatable Meson pkg-config files")
    if "lib/glib-2.0/include/glibconfig.h" not in names:
        raise ValueError("SDK is missing the target's GLib configuration header")
    output.mkdir()
    try:
        files = []
        for name, source, expected in selected:
            destination = output / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination)
            if digest(destination) != expected:
                raise ValueError("SDK input changed while copying")
            files.append({"path": name, "sha256": expected})
        (output / "sdk.json").write_text(
            json.dumps(
                {
                    "schemaVersion": 1,
                    "target": target,
                    "sourceCommit": ci["commit"],
                    "sourceManifestSha256": source_hash,
                    "files": files,
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
    except BaseException:
        shutil.rmtree(output)
        raise
