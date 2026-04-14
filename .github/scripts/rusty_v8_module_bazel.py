#!/usr/bin/env python3

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path


SHA256_RE = re.compile(r"[0-9a-f]{64}")
HTTP_FILE_BLOCK_RE = re.compile(r"(?ms)^http_file\(\n.*?^\)\n?")


class RustyV8ChecksumError(ValueError):
    pass


@dataclass(frozen=True)
class RustyV8HttpFile:
    start: int
    end: int
    block: str
    name: str
    downloaded_file_path: str
    sha256: str | None


def parse_checksum_manifest(path: Path) -> dict[str, str]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise RustyV8ChecksumError(f"missing checksum manifest: {path}") from exc

    checksums: dict[str, str] = {}
    for line_number, line in enumerate(lines, 1):
        if not line.strip():
            continue
        parts = line.split()
        if len(parts) != 2:
            raise RustyV8ChecksumError(
                f"{path}:{line_number}: expected '<sha256>  <filename>'"
            )
        checksum, filename = parts
        if not SHA256_RE.fullmatch(checksum):
            raise RustyV8ChecksumError(
                f"{path}:{line_number}: invalid SHA-256 digest for {filename}"
            )
        if not filename or filename in {".", ".."} or "/" in filename:
            raise RustyV8ChecksumError(
                f"{path}:{line_number}: expected a bare artifact filename"
            )
        if filename in checksums:
            raise RustyV8ChecksumError(
                f"{path}:{line_number}: duplicate checksum for {filename}"
            )
        checksums[filename] = checksum

    if not checksums:
        raise RustyV8ChecksumError(f"empty checksum manifest: {path}")
    return checksums


def string_field(block: str, field: str) -> str | None:
    # Matches one-line string fields inside http_file blocks, e.g. `sha256 = "...",`.
    match = re.search(rf'^\s*{re.escape(field)}\s*=\s*"([^"]+)",\s*$', block, re.M)
    if match:
        return match.group(1)
    return None


def rusty_v8_http_files(module_bazel: str, version: str) -> list[RustyV8HttpFile]:
    version_slug = version.replace(".", "_")
    name_prefix = f"rusty_v8_{version_slug}_"
    entries = []
    for match in HTTP_FILE_BLOCK_RE.finditer(module_bazel):
        block = match.group(0)
        name = string_field(block, "name")
        if not name or not name.startswith(name_prefix):
            continue
        downloaded_file_path = string_field(block, "downloaded_file_path")
        if not downloaded_file_path:
            raise RustyV8ChecksumError(
                f"MODULE.bazel {name} is missing downloaded_file_path"
            )
        entries.append(
            RustyV8HttpFile(
                start=match.start(),
                end=match.end(),
                block=block,
                name=name,
                downloaded_file_path=downloaded_file_path,
                sha256=string_field(block, "sha256"),
            )
        )
    return entries


def module_entry_set_errors(
    entries: list[RustyV8HttpFile],
    checksums: dict[str, str],
    version: str,
) -> list[str]:
    errors = []
    if not entries:
        errors.append(f"MODULE.bazel has no rusty_v8 http_file entries for {version}")
        return errors

    module_files: dict[str, RustyV8HttpFile] = {}
    duplicate_files = set()
    for entry in entries:
        if entry.downloaded_file_path in module_files:
            duplicate_files.add(entry.downloaded_file_path)
        module_files[entry.downloaded_file_path] = entry

    for filename in sorted(duplicate_files):
        errors.append(f"MODULE.bazel has duplicate http_file entries for {filename}")

    for filename in sorted(set(module_files) - set(checksums)):
        entry = module_files[filename]
        errors.append(f"MODULE.bazel {entry.name} has no checksum in the manifest")

    for filename in sorted(set(checksums) - set(module_files)):
        errors.append(f"manifest has {filename}, but MODULE.bazel has no http_file")

    return errors


def module_checksum_errors(
    entries: list[RustyV8HttpFile],
    checksums: dict[str, str],
) -> list[str]:
    errors = []
    for entry in entries:
        expected = checksums.get(entry.downloaded_file_path)
        if expected is None:
            continue
        if entry.sha256 is None:
            errors.append(f"MODULE.bazel {entry.name} is missing sha256")
        elif entry.sha256 != expected:
            errors.append(
                f"MODULE.bazel {entry.name} has sha256 {entry.sha256}, "
                f"expected {expected}"
            )
    return errors


def raise_checksum_errors(message: str, errors: list[str]) -> None:
    if errors:
        formatted_errors = "\n".join(f"- {error}" for error in errors)
        raise RustyV8ChecksumError(f"{message}:\n{formatted_errors}")


def check_module_bazel_text(
    module_bazel: str,
    checksums: dict[str, str],
    version: str,
) -> None:
    entries = rusty_v8_http_files(module_bazel, version)
    errors = [
        *module_entry_set_errors(entries, checksums, version),
        *module_checksum_errors(entries, checksums),
    ]
    raise_checksum_errors("rusty_v8 MODULE.bazel checksum drift", errors)


def block_with_sha256(block: str, checksum: str) -> str:
    sha256_line_re = re.compile(r'(?m)^(\s*)sha256\s*=\s*"[0-9a-f]+",\s*$')
    if sha256_line_re.search(block):
        return sha256_line_re.sub(
            lambda match: f'{match.group(1)}sha256 = "{checksum}",',
            block,
            count=1,
        )

    downloaded_file_path_match = re.search(
        r'(?m)^(\s*)downloaded_file_path\s*=\s*"[^"]+",\n',
        block,
    )
    if not downloaded_file_path_match:
        raise RustyV8ChecksumError("http_file block is missing downloaded_file_path")
    insert_at = downloaded_file_path_match.end()
    indent = downloaded_file_path_match.group(1)
    return f'{block[:insert_at]}{indent}sha256 = "{checksum}",\n{block[insert_at:]}'


def update_module_bazel_text(
    module_bazel: str,
    checksums: dict[str, str],
    version: str,
) -> str:
    entries = rusty_v8_http_files(module_bazel, version)
    errors = module_entry_set_errors(entries, checksums, version)
    raise_checksum_errors("cannot update rusty_v8 MODULE.bazel checksums", errors)

    updated = []
    previous_end = 0
    for entry in entries:
        updated.append(module_bazel[previous_end : entry.start])
        updated.append(
            block_with_sha256(entry.block, checksums[entry.downloaded_file_path])
        )
        previous_end = entry.end
    updated.append(module_bazel[previous_end:])
    return "".join(updated)


def check_module_bazel(
    module_bazel_path: Path,
    manifest_path: Path,
    version: str,
) -> None:
    checksums = parse_checksum_manifest(manifest_path)
    module_bazel = module_bazel_path.read_text(encoding="utf-8")
    check_module_bazel_text(module_bazel, checksums, version)
    print(f"{module_bazel_path} rusty_v8 {version} checksums match {manifest_path}")


def update_module_bazel(
    module_bazel_path: Path,
    manifest_path: Path,
    version: str,
) -> None:
    checksums = parse_checksum_manifest(manifest_path)
    module_bazel = module_bazel_path.read_text(encoding="utf-8")
    updated_module_bazel = update_module_bazel_text(module_bazel, checksums, version)
    if updated_module_bazel == module_bazel:
        print(f"{module_bazel_path} rusty_v8 {version} checksums are already current")
        return
    module_bazel_path.write_text(updated_module_bazel, encoding="utf-8")
    print(f"updated {module_bazel_path} rusty_v8 {version} checksums")
