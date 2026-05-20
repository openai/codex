"""Fetch ripgrep from the DotSlash manifest used by the npm package."""

import hashlib
import json
import shutil
import stat
import tarfile
import tempfile
import zipfile
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import urlopen

from .targets import REPO_ROOT
from .targets import TargetSpec
from .targets import resolve_input_path


RG_MANIFEST = REPO_ROOT / "codex-cli" / "bin" / "rg"
DOWNLOAD_TIMEOUT_SECS = 60


@dataclass(frozen=True)
class RgArtifact:
    size: int
    digest: str
    archive_format: str
    archive_member: str
    url: str


def resolve_rg_bin(spec: TargetSpec, rg_bin: Path | None) -> Path:
    if rg_bin is not None:
        return resolve_input_path(rg_bin, "ripgrep executable", "--rg-bin")

    return fetch_rg(spec)


def fetch_rg(
    spec: TargetSpec,
    *,
    manifest_path: Path = RG_MANIFEST,
    cache_root: Path | None = None,
) -> Path:
    artifact = artifact_for_target(spec, manifest_path)
    cache_dir = (cache_root or default_cache_root()) / f"{spec.target}-rg"
    archive_path = cache_dir / archive_filename(artifact.url)

    if not archive_is_valid(archive_path, artifact):
        download_archive(artifact.url, archive_path)
        try:
            verify_archive(archive_path, artifact)
        except RuntimeError:
            archive_path.unlink(missing_ok=True)
            raise

    dest = cache_dir / spec.rg_name
    extract_rg(archive_path, artifact, dest)
    if not spec.is_windows:
        mode = dest.stat().st_mode
        dest.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return dest


def artifact_for_target(spec: TargetSpec, manifest_path: Path) -> RgArtifact:
    manifest = load_manifest(manifest_path)
    try:
        platform_info = manifest["platforms"][spec.dotslash_platform]
    except KeyError as exc:
        raise RuntimeError(
            f"ripgrep manifest {manifest_path} is missing platform {spec.dotslash_platform!r}"
        ) from exc

    providers = platform_info.get("providers")
    if not providers:
        raise RuntimeError(
            f"ripgrep manifest {manifest_path} has no providers for {spec.dotslash_platform!r}"
        )

    hash_name = platform_info.get("hash")
    if hash_name != "sha256":
        raise RuntimeError(
            f"Unsupported ripgrep hash {hash_name!r} for "
            f"{spec.dotslash_platform!r}; expected sha256"
        )

    return RgArtifact(
        size=int(platform_info["size"]),
        digest=str(platform_info["digest"]),
        archive_format=str(platform_info["format"]),
        archive_member=str(platform_info["path"]),
        url=str(providers[0]["url"]),
    )


def load_manifest(manifest_path: Path) -> dict:
    text = manifest_path.read_text(encoding="utf-8")
    if text.startswith("#!"):
        text = "\n".join(text.splitlines()[1:])
    return json.loads(text)


def default_cache_root() -> Path:
    return Path(tempfile.gettempdir()) / "codex-package"


def archive_filename(url: str) -> str:
    filename = Path(urlparse(url).path).name
    if not filename:
        raise RuntimeError(f"Unable to determine archive filename from {url}")
    return filename


def archive_is_valid(archive_path: Path, artifact: RgArtifact) -> bool:
    if not archive_path.is_file():
        return False
    try:
        verify_archive(archive_path, artifact)
    except RuntimeError:
        archive_path.unlink(missing_ok=True)
        return False
    return True


def verify_archive(archive_path: Path, artifact: RgArtifact) -> None:
    actual_size = archive_path.stat().st_size
    if actual_size != artifact.size:
        raise RuntimeError(
            f"ripgrep archive {archive_path} has size {actual_size}, expected {artifact.size}"
        )

    digest = hashlib.sha256()
    with open(archive_path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            digest.update(chunk)

    actual_digest = digest.hexdigest()
    if actual_digest != artifact.digest:
        raise RuntimeError(
            f"ripgrep archive {archive_path} has sha256 {actual_digest}, "
            f"expected {artifact.digest}"
        )


def download_archive(url: str, archive_path: Path) -> None:
    archive_path.parent.mkdir(parents=True, exist_ok=True)
    temp_path = archive_path.with_suffix(f"{archive_path.suffix}.tmp")
    temp_path.unlink(missing_ok=True)
    try:
        with urlopen(url, timeout=DOWNLOAD_TIMEOUT_SECS) as response:
            with open(temp_path, "wb") as out:
                shutil.copyfileobj(response, out)
        temp_path.replace(archive_path)
    finally:
        temp_path.unlink(missing_ok=True)


def extract_rg(archive_path: Path, artifact: RgArtifact, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.unlink(missing_ok=True)

    if artifact.archive_format == "tar.gz":
        with tarfile.open(archive_path, "r:gz") as archive:
            try:
                member = archive.getmember(artifact.archive_member)
            except KeyError as exc:
                raise RuntimeError(
                    f"ripgrep archive {archive_path} is missing {artifact.archive_member!r}"
                ) from exc

            extracted = archive.extractfile(member)
            if extracted is None:
                raise RuntimeError(
                    f"ripgrep archive member {artifact.archive_member!r} is not a file"
                )
            with extracted, open(dest, "wb") as out:
                shutil.copyfileobj(extracted, out)
        return

    if artifact.archive_format == "zip":
        with zipfile.ZipFile(archive_path) as archive:
            try:
                with archive.open(artifact.archive_member) as extracted:
                    with open(dest, "wb") as out:
                        shutil.copyfileobj(extracted, out)
            except KeyError as exc:
                raise RuntimeError(
                    f"ripgrep archive {archive_path} is missing {artifact.archive_member!r}"
                ) from exc
        return

    raise RuntimeError(
        f"Unsupported ripgrep archive format {artifact.archive_format!r}; expected tar.gz or zip"
    )
