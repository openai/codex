"""Verify the pinned Cygwin build-input archive before offline installation."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import tarfile


def records(manifest):
    metadata = manifest["metadata"]
    if {item["file"] for item in metadata} != {
        "x86_64/setup.xz",
        "x86_64/setup.xz.sig",
    }:
        raise ValueError("signed setup metadata is required")
    entries = metadata + manifest["packages"]
    expected = {entry["file"]: entry for entry in entries}
    if len(expected) != len(entries):
        raise ValueError("duplicate Cygwin input")
    for name in expected:
        path = PurePosixPath(name)
        if (
            not path.parts
            or path.is_absolute()
            or ".." in path.parts
            or path.as_posix() != name
            or "\\" in name
            or ":" in name
        ):
            raise ValueError(f"unsafe Cygwin input: {name}")
    return expected


def extract(manifest, archive, destination):
    expected = records(manifest)
    pin = manifest["archive"]
    with archive.open("rb") as stream:
        if (
            archive.stat().st_size != pin["bytes"]
            or hashlib.file_digest(stream, "sha256").hexdigest() != pin["sha256"]
        ):
            raise ValueError("Cygwin archive digest or size mismatch")
        stream.seek(0)
        with tarfile.open(fileobj=stream, mode="r:gz") as bundle:
            members = bundle.getmembers()
            if len(members) != len(expected) or {m.name for m in members} != set(
                expected
            ):
                raise ValueError("missing, extra, or duplicate Cygwin inputs")
            for member in members:
                if not member.isfile() or member.issparse():
                    raise ValueError(f"non-regular Cygwin input: {member.name}")
                with bundle.extractfile(member) as source:
                    if (
                        member.size != expected[member.name]["bytes"]
                        or hashlib.file_digest(source, "sha512").hexdigest()
                        != expected[member.name]["sha512"]
                    ):
                        raise ValueError(f"Cygwin input mismatch: {member.name}")
            destination.mkdir(parents=True)
            try:
                bundle.extractall(destination, members=members, filter="data")
            except BaseException:
                shutil.rmtree(destination)
                raise


def check_installed(manifest, inventory):
    installed = sorted(
        tuple(line.split())
        for line in inventory.read_text().splitlines()
        if len(line.split()) == 2 and not line.startswith("Package ")
    )
    expected = sorted((item["name"], item["version"]) for item in manifest["packages"])
    if installed != expected:
        raise ValueError("installed Cygwin packages differ from reviewed inputs")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("extract", "check-installed"))
    parser.add_argument("--archive", type=Path)
    parser.add_argument("--directory", type=Path)
    parser.add_argument("--inventory", type=Path)
    args = parser.parse_args()
    pinned = json.loads(
        Path(__file__).with_name("voice-cygwin-snapshot.json").read_text()
    )
    if args.command == "extract":
        if args.archive is None or args.directory is None:
            parser.error("extract requires --archive and --directory")
        extract(pinned, args.archive, args.directory)
    else:
        if args.inventory is None:
            parser.error("check-installed requires --inventory")
        check_installed(pinned, args.inventory)
