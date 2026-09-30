"""Choose a forward-only npm alpha dist-tag under rust-release's concurrency lock."""

import json
from pathlib import Path
import subprocess
import sys
import tarfile

from releases import should_update_version


def npm_alpha_tag(tarball: Path, version: str, tag: str) -> str:
    # Read the actual package name: platform archives share @openai/codex,
    # while the SDK and proxy have independent alpha pointers.
    with tarfile.open(tarball, "r:gz") as archive:
        with archive.extractfile("package/package.json") as manifest:
            package = json.load(manifest)["name"]

    result = subprocess.run(
        ["npm", "view", package, "dist-tags", "--json", "--prefer-online"],
        check=True,
        capture_output=True,
        text=True,
    )
    current = json.loads(result.stdout).get(tag, "")
    if tag != "alpha":
        # build_npm_package.py appends -<platform> to the release version.
        current = current.removesuffix(f"-{tag.removeprefix('alpha-')}")
    if should_update_version(version, current):
        return tag

    # npm publish always writes a tag. Keep this version installable without
    # changing alpha, alpha-<platform>, or latest. Prefix avoids semver tags,
    # which npm rejects. Include the platform for versions of the same package.
    print(
        f"Keeping {package}@{tag} at {current}; publishing {version} separately",
        file=sys.stderr,
    )
    return f"release-{version}-{tag}"


if __name__ == "__main__":
    print(npm_alpha_tag(Path(sys.argv[1]), sys.argv[2], sys.argv[3]))
