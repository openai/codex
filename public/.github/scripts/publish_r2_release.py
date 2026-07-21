#!/usr/bin/env python3
"""Mirror a Codex GitHub Release to Cloudflare R2.

Cloudflare R2 exposes an S3-compatible API, so the built-in AWS CLI uses
standard AWS credentials and the R2 endpoint from ``AWS_ENDPOINT_URL``.
Objects are created under ``codex/releases/<version>/`` with a validated upload
checksum and checked using object metadata before the run succeeds. The
versioned prefix includes every release asset plus installer-facing
``release.json`` metadata derived from the verified downloads. Once those
objects are verified, the same metadata advances ``codex/channels/latest`` when
the release is marked latest and ``codex/channels/prerelease`` for prereleases.
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
from typing import Any, NamedTuple, NoReturn
from urllib.parse import quote

BUCKET = "releases"
PREFIX = "codex"
REPOSITORY = "openai/codex"
RELEASE_METADATA_NAME = "release.json"
VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:-(?:alpha|beta)(?:\.[0-9]+)?)?$")
CRC64_RE = re.compile(r"^[A-Za-z0-9+/]{11}=$")
SHA256_RE = re.compile(r"^sha256:([0-9a-f]{64})$")


class PublishError(RuntimeError):
    pass


class ReleaseAsset(NamedTuple):
    path: Path
    size: int
    sha256: str


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


def download_assets(tag: str, directory: Path) -> list[ReleaseAsset]:
    try:
        metadata = json.loads(
            run_command(
                [
                    "gh",
                    "release",
                    "view",
                    tag,
                    "--repo",
                    REPOSITORY,
                    "--json",
                    "assets",
                    "--jq",
                    "[.assets[] | {name, size, state, digest}]",
                ]
            )
        )
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
    except json.JSONDecodeError as error:
        raise PublishError(
            f"invalid GitHub release metadata for {tag}: {error}"
        ) from error

    expected = {}
    if not isinstance(metadata, list):
        raise PublishError(f"GitHub returned invalid release metadata for {tag}")
    for asset in metadata:
        if not isinstance(asset, dict):
            raise PublishError(
                f"GitHub returned invalid release metadata for {tag}: {asset!r}"
            )
        name = asset.get("name")
        size = asset.get("size")
        digest = asset.get("digest")
        match = SHA256_RE.fullmatch(digest) if isinstance(digest, str) else None
        if (
            not isinstance(name, str)
            or not name
            or name == RELEASE_METADATA_NAME
            or name in expected
            or type(size) is not int
            or size < 0
            or asset.get("state") != "uploaded"
            or match is None
        ):
            raise PublishError(
                f"GitHub returned invalid release metadata for {tag}: {asset!r}"
            )
        expected[name] = ReleaseAsset(directory / name, size, match.group(1))

    assets = sorted(directory.iterdir(), key=lambda path: path.name)
    if not assets:
        raise PublishError(f"GitHub Release {tag} has no assets")
    if any(not path.is_file() for path in assets) or {
        path.name for path in assets
    } != set(expected):
        raise PublishError("GitHub returned invalid release assets")
    return [expected[path.name] for path in assets]


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


def put_object(
    endpoint: str,
    key: str,
    path: Path,
    sha256: str,
    *,
    extra_args: list[str],
) -> None:
    try:
        run_command(
            [
                "aws",
                "s3",
                "cp",
                str(path),
                f"s3://{BUCKET}/{key}",
                *extra_args,
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
    parser.add_argument("--make-latest", choices=("true", "false"), required=True)
    parser.add_argument("--prerelease", choices=("true", "false"), required=True)
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
            assets = download_assets(args.tag, assets_directory)
            for asset in assets:
                with asset.path.open("rb") as source:
                    size, sha256 = stream_digest(source)
                if size != asset.size or sha256 != asset.sha256:
                    raise PublishError(
                        f"GitHub asset mismatch for {asset.path.name}: expected "
                        f"size={asset.size} sha256={asset.sha256}, got "
                        f"size={size} sha256={sha256}"
                    )
            for asset in assets:
                path = asset.path
                size = asset.size
                sha256 = asset.sha256
                key = f"{PREFIX}/releases/{version}/{path.name}"
                put_object(endpoint, key, path, sha256, extra_args=["--no-overwrite"])
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
            put_object(
                endpoint,
                metadata_key,
                metadata_path,
                metadata_sha256,
                extra_args=["--no-overwrite"],
            )
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
            channels = []
            if args.make_latest == "true":
                channels.append("latest")
            if args.prerelease == "true":
                channels.append("prerelease")
            for channel in channels:
                channel_key = f"{PREFIX}/channels/{channel}"
                put_object(
                    endpoint,
                    channel_key,
                    metadata_path,
                    metadata_sha256,
                    extra_args=["--content-type", "application/json"],
                )
                verify_remote(
                    endpoint,
                    channel_key,
                    metadata_size,
                    metadata_sha256,
                )
                print(
                    f"published and verified s3://{BUCKET}/{channel_key} "
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
