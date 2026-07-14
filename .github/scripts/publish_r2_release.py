#!/usr/bin/env python3
"""Mirror a Codex GitHub Release to Cloudflare R2.

Cloudflare R2 exposes an S3-compatible API, so the built-in AWS CLI uses
standard AWS credentials and the R2 endpoint from ``AWS_ENDPOINT_URL``.
Objects are created under ``codex/releases/<version>/`` with a validated upload
checksum and checked using object metadata before the run succeeds. The
versioned prefix includes every release asset plus installer-facing
``release.json`` metadata derived from the verified downloads.
"""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, NoReturn
from urllib.parse import quote

BUCKET = "releases"
PREFIX = "codex"
REPOSITORY = "openai/codex"
RELEASE_METADATA_NAME = "release.json"
VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:-(?:alpha|beta)(?:\.[0-9]+)?)?$")
CRC64_RE = re.compile(r"^[A-Za-z0-9+/]{11}=$")


class PublishError(RuntimeError):
    pass


def run_command(args: list[str]) -> str:
    result = subprocess.run(
        args,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.stdout:
        print(result.stdout, end="", file=sys.stderr)
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)
    result.check_returncode()
    return result.stdout or ""


def download_assets(tag: str, directory: Path) -> list[Path]:
    try:
        run_command(
            [
                "gh",
                "release",
                "download",
                tag,
                "--repo",
                REPOSITORY,
                "--dir",
                str(directory),
            ]
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise PublishError(
            f"GitHub release download failed for {tag}: {error}"
        ) from error

    assets = sorted(directory.iterdir(), key=lambda path: path.name)
    if not assets:
        raise PublishError(f"GitHub Release {tag} has no assets")
    if any(not path.is_file() or path.name == RELEASE_METADATA_NAME for path in assets):
        raise PublishError("GitHub returned invalid release assets")
    return assets


def stream_digest(source: Any) -> tuple[int, str]:
    digest = hashlib.sha256()
    size = 0
    while chunk := source.read(1024 * 1024):
        digest.update(chunk)
        size += len(chunk)
    return size, digest.hexdigest()


def raise_s3(
    action: str, key: str, error: Exception, detail: str | None = None
) -> NoReturn:
    raise PublishError(
        f"could not {action} s3://{BUCKET}/{key}: {detail or error}"
    ) from error


def put_immutable(endpoint: str, key: str, path: Path, sha256: str) -> None:
    try:
        run_command(
            [
                "aws",
                "s3",
                "cp",
                str(path),
                f"s3://{BUCKET}/{key}",
                "--no-overwrite",
                "--checksum-algorithm",
                "CRC64NVME",
                "--metadata",
                f"sha256={sha256}",
                "--endpoint-url",
                endpoint,
            ]
        )
    except subprocess.CalledProcessError as error:
        raise_s3("upload", key, error, (error.stderr or "").strip())
    except OSError as error:
        raise_s3("upload", key, error)


def verify_remote(
    endpoint: str,
    key: str,
    expected_size: int,
    expected_sha256: str,
) -> None:
    try:
        response = json.loads(
            run_command(
                [
                    "aws",
                    "s3api",
                    "head-object",
                    "--bucket",
                    BUCKET,
                    "--key",
                    key,
                    "--checksum-mode",
                    "ENABLED",
                    "--endpoint-url",
                    endpoint,
                ]
            )
        )
    except subprocess.CalledProcessError as error:
        raise_s3("inspect", key, error, (error.stderr or "").strip())
    except OSError as error:
        raise_s3("inspect", key, error)
    except json.JSONDecodeError as error:
        raise PublishError(f"invalid object metadata for {key}: {error}") from error

    metadata = response.get("Metadata") if isinstance(response, dict) else None
    size = response.get("ContentLength") if isinstance(response, dict) else None
    crc64 = response.get("ChecksumCRC64NVME") if isinstance(response, dict) else None
    sha256 = metadata.get("sha256") if isinstance(metadata, dict) else None
    if (
        size != expected_size
        or sha256 != expected_sha256
        or not isinstance(crc64, str)
        or not CRC64_RE.fullmatch(crc64)
    ):
        raise PublishError(
            f"object metadata mismatch for {key}: expected size={expected_size} "
            f"sha256={expected_sha256}, got size={size} sha256={sha256} "
            f"crc64nvme={crc64}"
        )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        endpoint = os.environ.get("AWS_ENDPOINT_URL")
        if not os.environ.get("GH_TOKEN"):
            raise PublishError("GH_TOKEN is required")
        if not endpoint:
            raise PublishError("AWS_ENDPOINT_URL is required for the R2 S3 endpoint")

        version = args.tag.removeprefix("rust-v")
        if args.tag == version or not VERSION_RE.fullmatch(version):
            raise PublishError(f"invalid rust release tag: {args.tag}")
        published = []
        metadata_assets = []
        with tempfile.TemporaryDirectory() as temp_dir:
            assets_directory = Path(temp_dir) / "assets"
            assets_directory.mkdir()
            for path in download_assets(args.tag, assets_directory):
                with path.open("rb") as source:
                    size, sha256 = stream_digest(source)
                key = f"{PREFIX}/releases/{version}/{path.name}"
                put_immutable(endpoint, key, path, sha256)
                verify_remote(endpoint, key, size, sha256)
                print(
                    f"published and verified s3://{BUCKET}/{key} "
                    f"size={size} sha256={sha256}",
                    file=sys.stderr,
                )
                published.append(
                    {
                        "key": key,
                        "sha256": sha256,
                        "size": size,
                    }
                )
                metadata_assets.append(
                    {
                        "name": path.name,
                        "digest": f"sha256:{sha256}",
                        "browser_download_url": (
                            f"https://releases.openai.com/{PREFIX}/releases/"
                            f"{version}/{quote(path.name, safe='')}"
                        ),
                    }
                )

            metadata_path = Path(temp_dir) / RELEASE_METADATA_NAME
            metadata_path.write_text(
                json.dumps(
                    {
                        "assets": metadata_assets,
                        "tag_name": args.tag,
                    },
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
            with metadata_path.open("rb") as source:
                metadata_size, metadata_sha256 = stream_digest(source)
            metadata_key = f"{PREFIX}/releases/{version}/{RELEASE_METADATA_NAME}"
            put_immutable(endpoint, metadata_key, metadata_path, metadata_sha256)
            verify_remote(
                endpoint,
                metadata_key,
                metadata_size,
                metadata_sha256,
            )
            print(
                f"published and verified s3://{BUCKET}/{metadata_key} "
                f"size={metadata_size} sha256={metadata_sha256}",
                file=sys.stderr,
            )

        print(
            json.dumps(
                {
                    "assetCount": len(published),
                    "assets": published,
                    "releaseMetadata": {
                        "key": metadata_key,
                        "sha256": metadata_sha256,
                        "size": metadata_size,
                    },
                    "releasePrefix": f"{PREFIX}/releases/{version}/",
                    "tag": args.tag,
                    "version": version,
                },
                sort_keys=True,
            )
        )
        return 0
    except PublishError as error:
        print(f"publish failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
