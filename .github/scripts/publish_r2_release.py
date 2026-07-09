#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "boto3>=1.43.39",
#     "ty==0.0.57",
# ]
# ///
"""Validate and publish Codex installer release assets.

The script finds the GitHub Release for a completed ``rust-release`` workflow,
downloads its seven installer assets, verifies the package checksums, then
publishes them to R2. It does not contact S3 until the downloads are verified.

The S3 client uses these standard AWS environment variables:

* ``AWS_ENDPOINT_URL`` (the R2 S3 endpoint)
* ``AWS_ACCESS_KEY_ID``
* ``AWS_SECRET_ACCESS_KEY``
* ``AWS_SESSION_TOKEN`` when the credential service issues one
* ``AWS_REGION`` or ``AWS_DEFAULT_REGION`` when required by the S3 client

This script constructs only ``codex/`` keys and calls HeadObject, GetObject,
and PutObject. It uploads the installer package archives and checksum manifest
to ``codex/releases/<version>/``, verifies each read-back, then publishes an
immutable version manifest. Exact existing objects are accepted so interrupted
runs can resume, but conflicting objects fail verification. The script does not
manage release channels.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import sys
import tempfile
import urllib.error
import urllib.parse
import urllib.request
from contextlib import closing
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn, cast

import boto3
from botocore.exceptions import BotoCoreError, ClientError


BUCKET = "releases"
PREFIX = "codex"
MANIFEST_NAME = "manifest.json"
SCHEMA_VERSION = 1
REPOSITORY = "openai/codex"
API_ROOT = "https://api.github.com"
API_VERSION = "2022-11-28"
VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:-(?:alpha|beta)(?:\.[0-9]+)?)?$")
CHECKSUM_RE = re.compile(r"^([0-9a-f]{64})  ([A-Za-z0-9_.-]+)$")
INSTALLER_ASSETS = (
    "codex-package-aarch64-apple-darwin.tar.gz",
    "codex-package-x86_64-apple-darwin.tar.gz",
    "codex-package-aarch64-unknown-linux-musl.tar.gz",
    "codex-package-x86_64-unknown-linux-musl.tar.gz",
    "codex-package-aarch64-pc-windows-msvc.tar.gz",
    "codex-package-x86_64-pc-windows-msvc.tar.gz",
    "codex-package_SHA256SUMS",
)
PACKAGE_ASSETS = INSTALLER_ASSETS[:-1]
CHECKSUM_ASSET = INSTALLER_ASSETS[-1]


class PublishError(RuntimeError):
    pass


@dataclass(frozen=True)
class Artifact:
    source: Path
    path: str
    key: str
    size: int
    sha256: str


@dataclass(frozen=True)
class ValidatedRelease:
    tag: str
    version: str


class BotoS3Client:
    """Small S3 adapter that leaves credential resolution to boto3."""

    def __init__(self, bucket: str = BUCKET) -> None:
        endpoint = os.environ.get("AWS_ENDPOINT_URL")
        if not endpoint:
            raise PublishError("AWS_ENDPOINT_URL is required for the R2 S3 endpoint")
        self.bucket = bucket
        region = os.environ.get("AWS_REGION") or os.environ.get("AWS_DEFAULT_REGION")
        self.client = boto3.client(
            "s3",
            endpoint_url=endpoint,
            region_name=region,
        )

    def exists(self, key: str) -> bool:
        try:
            self.client.head_object(Bucket=self.bucket, Key=key)
            return True
        except ClientError as error:
            code = str(error.response.get("Error", {}).get("Code", ""))
            if code in {"404", "NoSuchKey", "NotFound"}:
                return False
            self._raise("check", key, error)
        except BotoCoreError as error:
            self._raise("check", key, error)

    def put_file(self, key: str, path: Path) -> None:
        try:
            with path.open("rb") as body:
                self.client.put_object(
                    Bucket=self.bucket,
                    Key=key,
                    Body=body,
                    IfNoneMatch="*",
                )
        except (BotoCoreError, ClientError) as error:
            self._raise("upload", key, error)

    def put_bytes(
        self,
        key: str,
        contents: bytes,
        content_type: str,
    ) -> None:
        try:
            self.client.put_object(
                Bucket=self.bucket,
                Key=key,
                Body=contents,
                ContentType=content_type,
                IfNoneMatch="*",
            )
        except (BotoCoreError, ClientError) as error:
            self._raise("upload", key, error)

    def get_file(self, key: str, path: Path) -> None:
        try:
            response = self.client.get_object(Bucket=self.bucket, Key=key)
            with closing(response["Body"]), path.open("wb") as destination:
                shutil.copyfileobj(response["Body"], destination)
        except (BotoCoreError, ClientError) as error:
            self._raise("read back", key, error)

    def get_bytes(self, key: str) -> bytes:
        try:
            response = self.client.get_object(Bucket=self.bucket, Key=key)
            with closing(response["Body"]):
                return response["Body"].read()
        except (BotoCoreError, ClientError) as error:
            self._raise("read back", key, error)

    def _raise(self, action: str, key: str, error: Exception) -> NoReturn:
        raise PublishError(
            f"could not {action} s3://{self.bucket}/{key}: {error}"
        ) from error


class GitHubApi:
    def __init__(self, token: str) -> None:
        self.token = token

    def get_json(self, path: str) -> dict[str, object]:
        if not self.token:
            raise PublishError("GITHUB_TOKEN is required for GitHub API metadata")
        if not path.startswith("/"):
            raise PublishError(f"invalid GitHub API path: {path}")
        request = urllib.request.Request(
            f"{API_ROOT}{path}",
            headers={
                "Accept": "application/vnd.github+json",
                "Authorization": f"Bearer {self.token}",
                "User-Agent": "codex-r2-release-publisher",
                "X-GitHub-Api-Version": API_VERSION,
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                value = json.load(response)
        except (OSError, urllib.error.URLError, json.JSONDecodeError) as error:
            raise PublishError(
                f"GitHub API request failed for {path}: {error}"
            ) from error
        return require_object(value, f"GitHub response for {path}")

    def download_asset(
        self,
        url: str,
        destination: Path,
    ) -> None:
        request = urllib.request.Request(
            url,
            headers={
                "Accept": "application/octet-stream",
                "User-Agent": "codex-r2-release-publisher",
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                with destination.open("xb") as output:
                    while chunk := response.read(1024 * 1024):
                        output.write(chunk)
        except (OSError, urllib.error.URLError) as error:
            destination.unlink(missing_ok=True)
            raise PublishError(
                f"GitHub asset download failed for {destination.name}: {error}"
            ) from error


def require_object(value: object, label: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise PublishError(f"{label} must be an object")
    return cast(dict[str, object], value)


def require_list(value: object, label: str) -> list[object]:
    if not isinstance(value, list):
        raise PublishError(f"{label} must be a list")
    return cast(list[object], value)


def require_str(value: object, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise PublishError(f"{label} must be a non-empty string")
    return value


def release_asset_url(tag: str, name: str) -> str:
    return f"https://github.com/{REPOSITORY}/releases/download/{tag}/{name}"


def require_installer_assets(release: dict[str, object]) -> None:
    assets = require_list(release.get("assets"), "release assets")
    names = [
        require_str(require_object(asset, "release asset").get("name"), "asset name")
        for asset in assets
    ]
    for name in INSTALLER_ASSETS:
        count = names.count(name)
        if count != 1:
            raise PublishError(
                f"expected one release asset named {name}, found {count}"
            )


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_bytes(contents: bytes) -> str:
    return hashlib.sha256(contents).hexdigest()


def parse_checksums(path: Path) -> dict[str, str]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise PublishError(f"could not read checksum manifest: {error}") from error
    checksums: dict[str, str] = {}
    for line_number, line in enumerate(lines, start=1):
        match = CHECKSUM_RE.fullmatch(line)
        if not match:
            raise PublishError(f"invalid checksum manifest line {line_number}")
        digest, name = match.groups()
        if name in checksums:
            raise PublishError(f"duplicate checksum manifest entry: {name}")
        checksums[name] = digest
    return checksums


def verify_package_checksums(dist: Path) -> None:
    checksums = parse_checksums(dist / CHECKSUM_ASSET)
    for name in PACKAGE_ASSETS:
        if checksums.get(name) != sha256_file(dist / name):
            raise PublishError(f"checksum mismatch for release asset {name}")


def validate_release(
    event: dict[str, object],
    api: GitHubApi,
) -> ValidatedRelease:
    run = require_object(event.get("workflow_run"), "workflow_run")
    tag = require_str(run.get("head_branch"), "workflow run tag")
    version = tag.removeprefix("rust-v")
    if tag == version or not VERSION_RE.fullmatch(version):
        raise PublishError(f"invalid rust release tag: {tag}")
    quoted_tag = urllib.parse.quote(tag, safe="")
    release = api.get_json(f"/repos/{REPOSITORY}/releases/tags/{quoted_tag}")
    require_installer_assets(release)
    return ValidatedRelease(tag=tag, version=version)


def load_event(path: Path) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise PublishError(f"could not read workflow_run event: {error}") from error
    return require_object(value, "workflow_run event")


def download_release_assets(
    release: ValidatedRelease, dist: Path, api: GitHubApi
) -> None:
    try:
        dist.mkdir(parents=True, exist_ok=False)
    except OSError as error:
        raise PublishError(
            f"could not create isolated dist directory: {error}"
        ) from error
    for name in INSTALLER_ASSETS:
        api.download_asset(release_asset_url(release.tag, name), dist / name)
    verify_package_checksums(dist)


def validate_version(version: str) -> None:
    if not VERSION_RE.fullmatch(version):
        raise PublishError(f"invalid Codex release version: {version}")


def release_root(version: str) -> str:
    return f"{PREFIX}/releases/{version}"


def artifacts_for(dist: Path, version: str) -> list[Artifact]:
    if not dist.is_dir():
        raise PublishError(f"dist directory does not exist: {dist}")

    root = release_root(version)
    artifacts = []
    for name in INSTALLER_ASSETS:
        matches = sorted(path for path in dist.rglob(name) if path.is_file())
        if len(matches) != 1:
            raise PublishError(
                f"expected exactly one installer asset named {name}, found {len(matches)}"
            )
        path = matches[0]
        if path.is_symlink():
            raise PublishError(f"refusing symlink in dist: {path}")
        artifacts.append(
            Artifact(
                source=path,
                path=name,
                key=f"{root}/{name}",
                size=path.stat().st_size,
                sha256=sha256_file(path),
            )
        )
    return artifacts


def manifest_bytes(version: str, artifacts: list[Artifact]) -> bytes:
    manifest = {
        "artifacts": [
            {
                "key": artifact.key,
                "path": artifact.path,
                "sha256": artifact.sha256,
                "size": artifact.size,
            }
            for artifact in artifacts
        ],
        "product": "codex",
        "releasePrefix": f"{release_root(version)}/",
        "schemaVersion": SCHEMA_VERSION,
        "version": version,
    }
    return (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()


def verify_file(client: BotoS3Client, key: str, expected: Artifact) -> None:
    with tempfile.TemporaryDirectory() as temp_dir:
        downloaded = Path(temp_dir) / "readback"
        client.get_file(key, downloaded)
        actual_size = downloaded.stat().st_size
        actual_sha256 = sha256_file(downloaded)
    if actual_size != expected.size or actual_sha256 != expected.sha256:
        raise PublishError(
            f"read-back mismatch for {key}: expected size={expected.size} "
            f"sha256={expected.sha256}, got size={actual_size} sha256={actual_sha256}"
        )


def verify_bytes(client: BotoS3Client, key: str, expected: bytes) -> None:
    actual = client.get_bytes(key)
    if actual != expected:
        raise PublishError(
            f"read-back mismatch for {key}: expected size={len(expected)} "
            f"sha256={sha256_bytes(expected)}, got size={len(actual)} "
            f"sha256={sha256_bytes(actual)}"
        )


def publish(dist: Path, version: str, client: BotoS3Client) -> dict[str, object]:
    validate_version(version)
    artifacts = artifacts_for(dist, version)

    for artifact in artifacts:
        if not client.exists(artifact.key):
            client.put_file(artifact.key, artifact.source)
        verify_file(client, artifact.key, artifact)

    manifest_key = f"{release_root(version)}/{MANIFEST_NAME}"
    manifest = manifest_bytes(version, artifacts)
    if not client.exists(manifest_key):
        client.put_bytes(
            manifest_key,
            manifest,
            content_type="application/json",
        )
    verify_bytes(client, manifest_key, manifest)

    return {
        "artifacts": len(artifacts),
        "manifestKey": manifest_key,
        "manifestSha256": sha256_bytes(manifest),
        "version": version,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        api = GitHubApi(os.environ.get("GITHUB_TOKEN", ""))
        release = validate_release(load_event(args.event), api)
        with tempfile.TemporaryDirectory() as temp_dir:
            dist = Path(temp_dir) / "dist"
            download_release_assets(release, dist, api)
            receipt = publish(dist, release.version, BotoS3Client())
        receipt["tag"] = release.tag
    except PublishError as error:
        print(f"publish failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
