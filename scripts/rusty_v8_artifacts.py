"""Manifest for Codex-built rusty_v8 artifacts."""

from __future__ import annotations

import re
import tomllib
from dataclasses import dataclass
from pathlib import Path
from pathlib import PurePosixPath


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST_PATH = REPO_ROOT / "third_party" / "v8" / "artifacts.toml"
VERSION_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
V8_VERSION_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$")
COMMIT_PATTERN = re.compile(r"^[0-9a-f]{40}$")
EXPECTED_KEYS = {
    "artifact_identity",
    "patch_recipe",
    "patches",
    "schema_version",
    "v8_source_commit",
    "v8_version",
    "wrapper_v8_version",
    "wrapper_version",
}


@dataclass(frozen=True)
class RustyV8ArtifactManifest:
    schema_version: int
    wrapper_version: str
    wrapper_v8_version: str
    v8_version: str
    v8_source_commit: str
    patch_recipe: int
    artifact_identity: str
    patches: tuple[PurePosixPath, ...]

    @classmethod
    def load(
        cls,
        manifest_path: Path = DEFAULT_MANIFEST_PATH,
        *,
        repo_root: Path = REPO_ROOT,
    ) -> RustyV8ArtifactManifest:
        data = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        keys = set(data)
        if keys != EXPECTED_KEYS:
            missing = sorted(EXPECTED_KEYS - keys)
            unexpected = sorted(keys - EXPECTED_KEYS)
            raise ValueError(
                "invalid rusty_v8 artifact manifest keys; "
                f"missing={missing}, unexpected={unexpected}"
            )

        manifest = cls(
            schema_version=data["schema_version"],
            wrapper_version=data["wrapper_version"],
            wrapper_v8_version=data["wrapper_v8_version"],
            v8_version=data["v8_version"],
            v8_source_commit=data["v8_source_commit"],
            patch_recipe=data["patch_recipe"],
            artifact_identity=data["artifact_identity"],
            patches=tuple(PurePosixPath(path) for path in data["patches"]),
        )
        manifest.validate(repo_root)
        return manifest

    def expected_artifact_identity(self) -> str:
        return (
            f"rusty-v8-v{self.wrapper_version}-v8-{self.v8_version}"
            f"-recipe-{self.patch_recipe}"
        )

    def patch_paths(self, repo_root: Path = REPO_ROOT) -> tuple[Path, ...]:
        return tuple(repo_root / path for path in self.patches)

    def validate(self, repo_root: Path) -> None:
        if self.schema_version != 1:
            raise ValueError(
                f"unsupported rusty_v8 artifact manifest schema: {self.schema_version}"
            )
        if not isinstance(self.wrapper_version, str) or not VERSION_PATTERN.fullmatch(
            self.wrapper_version
        ):
            raise ValueError(
                f"invalid rusty_v8 wrapper version: {self.wrapper_version}"
            )
        if not isinstance(
            self.wrapper_v8_version, str
        ) or not V8_VERSION_PATTERN.fullmatch(self.wrapper_v8_version):
            raise ValueError(f"invalid wrapper V8 version: {self.wrapper_v8_version}")
        if not isinstance(self.v8_version, str) or not V8_VERSION_PATTERN.fullmatch(
            self.v8_version
        ):
            raise ValueError(f"invalid V8 engine version: {self.v8_version}")
        wrapper_line = self.wrapper_v8_version.rsplit(".", 1)[0]
        engine_line = self.v8_version.rsplit(".", 1)[0]
        if wrapper_line != engine_line:
            raise ValueError(
                f"independent V8 updates must stay on wrapper patch line "
                f"{wrapper_line}; found {self.v8_version}"
            )
        if not isinstance(self.v8_source_commit, str) or not COMMIT_PATTERN.fullmatch(
            self.v8_source_commit
        ):
            raise ValueError(f"invalid V8 source commit: {self.v8_source_commit}")
        if not isinstance(self.patch_recipe, int) or self.patch_recipe < 1:
            raise ValueError(f"invalid V8 patch recipe: {self.patch_recipe}")
        expected_identity = self.expected_artifact_identity()
        if self.artifact_identity != expected_identity:
            raise ValueError(
                f"artifact identity {self.artifact_identity!r} does not match "
                f"{expected_identity!r}"
            )
        if not self.patches:
            raise ValueError("the V8 patch recipe must contain at least one patch")
        if len(set(self.patches)) != len(self.patches):
            raise ValueError("the V8 patch recipe contains duplicate paths")

        recipe_root = PurePosixPath(
            f"third_party/v8/patches/recipe-{self.patch_recipe}"
        )
        for patch in self.patches:
            if (
                patch.is_absolute()
                or ".." in patch.parts
                or patch.suffix != ".patch"
                or not patch.is_relative_to(recipe_root)
            ):
                raise ValueError(f"patch {patch} is outside recipe {self.patch_recipe}")
            if not (repo_root / patch).is_file():
                raise ValueError(f"missing V8 patch: {patch}")
