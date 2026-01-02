#!/usr/bin/env python3
"""Upload release artifacts to Azure Blob Storage (bbb/azcopy).

Manual upload mode
------------------
Upload an existing file/dir from your machine.

Release mode
------------
`--release-codex` builds and uploads:
- `codex-tui-YYYY-MM-DD` (+ `.sha256`)
- `codex-google-workspace-mcp-YYYY-MM-DD.tgz` (+ `.sha256`)

Destination
-----------
Prefer `--dest-base`:
  --dest-base az://oaiphx8/oaikhai/codex/
or:
  --dest-base https://<account>.blob.core.windows.net/<container>/<prefix>/

Auth
----
- For `bbb`, use Azure auth (e.g. `az login`) or pass a SAS token if using https.
- For `azcopy`, use `azcopy login` / Azure CLI or pass a SAS token if using https.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from datetime import date
from pathlib import Path
from urllib.parse import urlparse


DEFAULT_ACCOUNT_URL = "https://oaiartifacts.blob.core.windows.net"
DEFAULT_PREFIX = "khai"
DEFAULT_TOOL = "bbb"

DEFAULT_CODEX_RELEASE_DEST_BASE = "az://oaiphx8/oaikhai/codex/"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)

    parser.add_argument(
        "--release-codex",
        action="store_true",
        help="Build and upload codex-tui + google workspace MCP tarball.",
    )

    parser.add_argument(
        "--src",
        type=Path,
        default=None,
        help="File or directory to upload (manual upload mode).",
    )

    parser.add_argument(
        "--tool",
        choices=("bbb", "azcopy"),
        default=DEFAULT_TOOL,
        help=f"Uploader tool to use (default: {DEFAULT_TOOL}).",
    )
    parser.add_argument(
        "--concurrency",
        type=int,
        default=None,
        help="Concurrency to pass to bbb (ignored for azcopy).",
    )

    parser.add_argument(
        "--dest-base",
        default=None,
        help=(
            "Destination directory URL (recommended). Example: "
            "'az://oaiphx8/oaikhai/codex/' or "
            "'https://<account>.blob.core.windows.net/<container>/<prefix>/'."
        ),
    )

    # Legacy destination composition when --dest-base is omitted.
    parser.add_argument(
        "--account-url",
        default=DEFAULT_ACCOUNT_URL,
        help=f"Storage account URL (default: {DEFAULT_ACCOUNT_URL}).",
    )
    parser.add_argument(
        "--container",
        default=None,
        help="Azure Blob container name (required if --dest-base is not provided).",
    )
    parser.add_argument(
        "--prefix",
        default=DEFAULT_PREFIX,
        help=f"Remote prefix under the container (default: {DEFAULT_PREFIX}).",
    )

    parser.add_argument(
        "--label",
        default=None,
        help="Release label used in names (default: YYYY-MM-DD).",
    )
    parser.add_argument(
        "--dest-name",
        default=None,
        help=(
            "Destination file name (single-file uploads only). When set, the file is "
            "uploaded to <dest-base>/<dest-name> (no <label>/ directory)."
        ),
    )
    parser.add_argument(
        "--codex-tui",
        action="store_true",
        help=(
            "Convenience mode for Codex TUI uploads (manual mode): defaults --prefix=codex and "
            "defaults --dest-name to codex-tui-<label><ext>."
        ),
    )

    parser.add_argument(
        "--sas",
        default=None,
        help=(
            "SAS token for the destination (without leading '?'). Only used for https destinations."
        ),
    )

    parser.add_argument(
        "--include-manifest",
        action="store_true",
        help="Also upload a generated manifest.json with sha256/size for each file (dirs only).",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print what would be uploaded, but do not upload.",
    )

    return parser.parse_args()


def main() -> int:
    args = parse_args()

    uploader = resolve_uploader(args.tool)
    if uploader is None:
        if args.tool == "bbb":
            print("error: `bbb` is required (boostedblob).", file=sys.stderr)
        else:
            print("error: `azcopy` is required. Install: https://aka.ms/azcopy", file=sys.stderr)
        return 2

    label = (args.label or date.today().isoformat()).strip() or date.today().isoformat()

    if args.release_codex and args.dest_base is None and args.container is None:
        args.dest_base = DEFAULT_CODEX_RELEASE_DEST_BASE

    dest_base = compute_dest_base(
        dest_base=args.dest_base,
        account_url=args.account_url,
        container=args.container,
        prefix=("codex" if args.codex_tui and args.prefix == DEFAULT_PREFIX else args.prefix),
        sas=args.sas,
    )

    if args.release_codex:
        if args.dry_run:
            print(f"Destination: {redact_sas(dest_base)}")
            for name in [
                f"codex-tui-{label}",
                f"codex-tui-{label}.sha256",
                f"codex-google-workspace-mcp-{label}.tgz",
                f"codex-google-workspace-mcp-{label}.tgz.sha256",
            ]:
                print(f"DRY-RUN: <built> -> {redact_sas(join_url(dest_base, name))}")
            return 0

        release_codex(
            uploader=uploader,
            tool=args.tool,
            dest_base=dest_base,
            label=label,
            concurrency=args.concurrency,
        )
        return 0

    if args.src is None:
        print("error: --src is required unless --release-codex is set.", file=sys.stderr)
        return 2

    src = args.src.expanduser().resolve()
    if not src.exists():
        print(f"error: source path not found: {src}", file=sys.stderr)
        return 2

    if args.dest_name is not None and not src.is_file():
        print("error: --dest-name is only valid when --src is a file.", file=sys.stderr)
        return 2

    if args.include_manifest and src.is_file():
        print("error: --include-manifest is only supported when --src is a directory.", file=sys.stderr)
        return 2

    dest_name = args.dest_name
    if args.codex_tui and src.is_file() and dest_name is None:
        dest_name = f"codex-tui-{label}{full_suffix(src)}"

    if dest_name is None:
        dest_dir_url = join_url(dest_base, f"{label}/")
        dest_file_url: str | None = None
    else:
        dest_dir_url = dest_base
        dest_file_url = join_url(dest_base, dest_name)

    if args.dry_run:
        if src.is_file():
            final = dest_file_url or join_url(dest_dir_url, src.name)
            print(f"Destination: {redact_sas(final)}")
            print(f"DRY-RUN: {src} -> {redact_sas(final)}")
        else:
            print(f"Destination: {redact_sas(dest_dir_url)}")
            print(f"DRY-RUN: {src}/ -> {redact_sas(dest_dir_url)}")
            if args.include_manifest:
                print(f"DRY-RUN: (generated manifest) -> {redact_sas(join_url(dest_dir_url, 'manifest.json'))}")
        return 0

    manifest_temp: Path | None = None
    try:
        if args.include_manifest and src.is_dir():
            manifest_temp = Path(tempfile.mkdtemp(prefix="oaiartifacts-manifest-")) / "manifest.json"
            manifest = build_manifest(src)
            manifest_temp.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

        print(f"Uploading to {redact_sas(dest_file_url or dest_dir_url)} (tool: {args.tool})")
        if args.tool == "bbb":
            upload_with_bbb(
                bbb=uploader,
                src=src,
                dest_dir_url=dest_dir_url,
                dest_file_url=dest_file_url,
                concurrency=args.concurrency,
                manifest_path=manifest_temp,
            )
        else:
            upload_with_azcopy(
                azcopy=uploader,
                src=src,
                dest_dir_url=dest_dir_url,
                dest_file_url=dest_file_url,
                manifest_path=manifest_temp,
            )
    finally:
        if manifest_temp is not None:
            shutil.rmtree(manifest_temp.parent, ignore_errors=True)

    print(f"Uploaded {src}")
    return 0


def release_codex(*, uploader: str, tool: str, dest_base: str, label: str, concurrency: int | None) -> None:
    repo_root = Path(__file__).resolve().parent.parent

    cargo = shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("cargo not found on PATH")

    pnpm = shutil.which("pnpm")
    if pnpm is None:
        raise RuntimeError("pnpm not found on PATH")

    codex_tui_binary = build_codex_tui(repo_root, cargo=cargo)
    codex_tui_name = f"codex-tui-{label}"
    codex_tui_url = join_url(dest_base, codex_tui_name)
    print(f"Uploading Codex TUI: {codex_tui_binary} -> {redact_sas(codex_tui_url)}")
    upload_file(tool=tool, uploader=uploader, src=codex_tui_binary, dst_url=codex_tui_url, concurrency=concurrency)

    with tempfile.TemporaryDirectory(prefix="codex-release-") as temp_dir:
        temp_dir_path = Path(temp_dir)

        codex_tui_sha = sha256_file(codex_tui_binary)
        codex_tui_sha_path = temp_dir_path / f"{codex_tui_name}.sha256"
        codex_tui_sha_path.write_text(f"{codex_tui_sha}\n", encoding="utf-8")
        codex_tui_sha_url = join_url(dest_base, f"{codex_tui_name}.sha256")
        print(f"Uploading Codex TUI sha256: {codex_tui_sha_path} -> {redact_sas(codex_tui_sha_url)}")
        upload_file(tool=tool, uploader=uploader, src=codex_tui_sha_path, dst_url=codex_tui_sha_url, concurrency=concurrency)

        mcp_tgz_path = build_google_workspace_mcp_tgz(repo_root, pnpm=pnpm, out_dir=temp_dir_path)
        mcp_name = f"codex-google-workspace-mcp-{label}.tgz"
        final_mcp_tgz = temp_dir_path / mcp_name
        shutil.copy2(mcp_tgz_path, final_mcp_tgz)
        mcp_url = join_url(dest_base, mcp_name)
        print(f"Uploading google-workspace-mcp: {final_mcp_tgz} -> {redact_sas(mcp_url)}")
        upload_file(tool=tool, uploader=uploader, src=final_mcp_tgz, dst_url=mcp_url, concurrency=concurrency)

        mcp_sha_path = temp_dir_path / f"{mcp_name}.sha256"
        mcp_sha_path.write_text(f"{sha256_file(final_mcp_tgz)}\n", encoding="utf-8")
        mcp_sha_url = join_url(dest_base, f"{mcp_name}.sha256")
        print(f"Uploading google-workspace-mcp sha256: {mcp_sha_path} -> {redact_sas(mcp_sha_url)}")
        upload_file(tool=tool, uploader=uploader, src=mcp_sha_path, dst_url=mcp_sha_url, concurrency=concurrency)


def build_codex_tui(repo_root: Path, *, cargo: str) -> Path:
    codex_rs = repo_root / "codex-rs"
    binary = codex_rs / "target" / "release" / ("codex-tui.exe" if os.name == "nt" else "codex-tui")
    run_command([cargo, "build", "-p", "codex-tui", "--release"], cwd=codex_rs)
    if not binary.exists():
        raise RuntimeError(f"Expected Codex TUI binary not found at {binary}")
    return binary


def build_google_workspace_mcp_tgz(repo_root: Path, *, pnpm: str, out_dir: Path) -> Path:
    pkg_root = repo_root / "google-workspace-mcp"
    if not pkg_root.exists():
        raise RuntimeError(f"Missing google-workspace-mcp directory at {pkg_root}")

    if not (pkg_root / "node_modules").exists():
        run_command([pnpm, "install", "--frozen-lockfile"], cwd=repo_root)

    run_command([pnpm, "run", "build"], cwd=pkg_root)
    run_command([pnpm, "pack", "--pack-destination", str(out_dir)], cwd=pkg_root)

    tgz_files = sorted(out_dir.glob("*.tgz"))
    if not tgz_files:
        raise RuntimeError(f"pnpm pack produced no tgz files in {out_dir}")
    return tgz_files[-1]


def compute_dest_base(*, dest_base: str | None, account_url: str, container: str | None, prefix: str, sas: str | None) -> str:
    if dest_base is not None:
        base = dest_base.strip()
        if not base:
            raise RuntimeError("--dest-base may not be empty when provided.")
        if not base.endswith("/"):
            base += "/"
        if sas and base.startswith("https://"):
            base = append_sas(base, sas)
        return base

    if container is None:
        raise RuntimeError("--container is required when --dest-base is not provided.")

    account_url = account_url.rstrip("/")
    validate_account_url(account_url)

    prefix = normalize_prefix(prefix)
    base = f"{account_url}/{container.strip('/')}/{prefix}"
    if not base.endswith("/"):
        base += "/"
    base = append_sas(base, sas)
    return base


def normalize_prefix(prefix: str) -> str:
    prefix = prefix.strip().strip("/")
    if prefix:
        return f"{prefix}/"
    return ""


def validate_account_url(account_url: str) -> None:
    parsed = urlparse(account_url)
    if parsed.scheme != "https" or not parsed.netloc:
        raise RuntimeError(f"Invalid --account-url: {account_url}")


def append_sas(url: str, sas: str | None) -> str:
    if not sas:
        return url
    sas = sas.strip()
    if not sas:
        return url
    if sas.startswith("?"):
        sas = sas[1:]
    separator = "&" if "?" in url else "?"
    return f"{url}{separator}{sas}"


def redact_sas(url: str) -> str:
    if "?" not in url:
        return url
    return url.split("?", 1)[0] + "?<redacted>"


def join_url(base: str, suffix: str) -> str:
    if not base.endswith("/"):
        base = f"{base}/"
    if suffix.startswith("/"):
        suffix = suffix[1:]
    return f"{base}{suffix}"


def iter_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for path in root.rglob("*"):
        if path.is_file():
            files.append(path)
    files.sort()
    return files


def build_manifest(root: Path) -> dict:
    entries: list[dict] = []
    for path in iter_files(root):
        rel = path.relative_to(root).as_posix()
        entries.append(
            {
                "path": rel,
                "bytes": path.stat().st_size,
                "sha256": sha256_file(path),
            }
        )
    return {"root": root.name, "files": entries}


def sha256_file(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def run_command(cmd: list[str], *, cwd: Path | None) -> None:
    print("+", " ".join(cmd))
    subprocess.run(cmd, check=True, cwd=str(cwd) if cwd is not None else None)


def resolve_uploader(tool: str) -> str | None:
    return shutil.which(tool)


def upload_file(*, tool: str, uploader: str, src: Path, dst_url: str, concurrency: int | None) -> None:
    if tool == "bbb":
        cmd = [uploader, "cp"]
        if concurrency is not None:
            cmd.extend(["--concurrency", str(concurrency)])
        cmd.extend([str(src), dst_url])
        run_command(cmd, cwd=None)
        return

    run_command(
        [
            uploader,
            "copy",
            str(src),
            dst_url,
            "--overwrite=ifSourceNewer",
        ],
        cwd=None,
    )


def upload_with_bbb(
    *,
    bbb: str,
    src: Path,
    dest_dir_url: str,
    dest_file_url: str | None,
    concurrency: int | None,
    manifest_path: Path | None,
) -> None:
    if src.is_dir():
        cmd = [bbb, "sync"]
        if concurrency is not None:
            cmd.extend(["--concurrency", str(concurrency)])
        cmd.extend([str(src), dest_dir_url])
        run_command(cmd, cwd=None)
    else:
        cmd = [bbb, "cp"]
        if concurrency is not None:
            cmd.extend(["--concurrency", str(concurrency)])
        cmd.extend([str(src), dest_file_url or join_url(dest_dir_url, src.name)])
        run_command(cmd, cwd=None)

    if manifest_path is not None:
        cmd = [bbb, "cp"]
        if concurrency is not None:
            cmd.extend(["--concurrency", str(concurrency)])
        cmd.extend([str(manifest_path), join_url(dest_dir_url, "manifest.json")])
        run_command(cmd, cwd=None)


def upload_with_azcopy(
    *,
    azcopy: str,
    src: Path,
    dest_dir_url: str,
    dest_file_url: str | None,
    manifest_path: Path | None,
) -> None:
    if src.is_dir():
        run_command(
            [
                azcopy,
                "copy",
                str(src),
                dest_dir_url,
                "--recursive=true",
                "--overwrite=ifSourceNewer",
            ],
            cwd=None,
        )
    else:
        run_command(
            [
                azcopy,
                "copy",
                str(src),
                dest_file_url or join_url(dest_dir_url, src.name),
                "--overwrite=ifSourceNewer",
            ],
            cwd=None,
        )

    if manifest_path is not None:
        run_command(
            [
                azcopy,
                "copy",
                str(manifest_path),
                join_url(dest_dir_url, "manifest.json"),
                "--overwrite=ifSourceNewer",
            ],
            cwd=None,
        )


def full_suffix(path: Path) -> str:
    suffixes = path.suffixes
    if not suffixes:
        return ""
    return "".join(suffixes)


if __name__ == "__main__":
    raise SystemExit(main())
