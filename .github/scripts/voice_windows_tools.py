"""Expose verified Windows build-tool installations as local Bazel inputs.

This declares the installed support trees; the caller authenticates their
inputs and verifies the installed package inventory first.
"""

import argparse
import itertools
import json
from pathlib import Path
import stat


def export_repository(root: Path, target: str, pkg_config: Path):
    if target not in ("x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"):
        raise ValueError("unsupported Windows target")
    root = root.resolve(strict=True)
    pkg_config = pkg_config.resolve(strict=True)
    names = {
        "shell": "cygwin/bin/bash.exe",
        "make": "cygwin/bin/make.exe",
        "cygpath": "cygwin/bin/cygpath.exe",
        "automake": "cygwin/bin/automake-1.18",
        "pkg_config": pkg_config.relative_to(root).as_posix(),
    }
    if not names["pkg_config"].startswith("pkgconf-image/"):
        raise ValueError("pkg-config must belong to the extracted image")
    outputs = ("BUILD.bazel", "MODULE.bazel", "voice-tools.json")
    if any((root / name).exists() for name in outputs):
        raise ValueError("Bazel tool repository outputs must be fresh")
    # A nested package boundary would silently remove files from Bazel's glob.
    # Reparse points could escape the installation or hide support directories.
    for directory in ("cygwin", "pkgconf-image"):
        tree = root / directory
        if not tree.is_dir():
            raise ValueError(f"missing tool tree: {directory}")
        for path in itertools.chain((tree,), tree.rglob("*")):
            info = path.lstat()
            if (
                stat.S_ISLNK(info.st_mode)
                or getattr(info, "st_file_attributes", 0)
                & stat.FILE_ATTRIBUTE_REPARSE_POINT
                or not (stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode))
                or path.name.casefold() in ("build", "build.bazel")
            ):
                raise ValueError(f"unsupported tool-tree entry: {path}")
    if any(not (root / name).is_file() for name in names.values()):
        raise ValueError("required Windows build tool missing")
    definitions = [
        'package(default_visibility = ["//visibility:public"])',
        'filegroup(name = "cygwin", srcs = glob(["cygwin/**"], allow_empty = False))',
        'filegroup(name = "pkgconf", srcs = glob(["pkgconf-image/**"], allow_empty = False))',
        'filegroup(name = "tools", srcs = [":cygwin", ":pkgconf", "voice-tools.json"])',
    ]
    definitions.extend(
        f"filegroup(name = {json.dumps(name)}, srcs = [{json.dumps(path)}])"
        for name, path in names.items()
    )
    metadata = {
        "schemaVersion": 1,
        "target": target,
        "cygwinArchitecture": "x86_64",
        "tools": names,
    }
    for name, contents in (
        ("BUILD.bazel", "\n".join(definitions) + "\n"),
        ("MODULE.bazel", 'module(name = "voice_windows_tools")\n'),
        ("voice-tools.json", json.dumps(metadata, indent=2) + "\n"),
    ):
        with (root / name).open("x", encoding="utf-8") as output:
            output.write(contents)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--pkg-config", type=Path, required=True)
    args = parser.parse_args()
    export_repository(args.root, args.target, args.pkg_config)
