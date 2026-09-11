"""Add pinned, unmodified Microsoft retail CRT files to Windows release staging."""

import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import urllib.request
import zipfile

from windows_runtime import EXTERNAL_IMPORTS, inspect


def stage(root: Path, target: str, helper: Path):
    pin = json.loads(Path(__file__).with_name("windows-crt.json").read_text())[target]
    with urllib.request.urlopen(pin["url"], timeout=90) as response:
        archive = response.read(6 * 1024 * 1024 + 1)
    if hashlib.sha256(archive).hexdigest() != pin["sha256"]:
        raise ValueError("Microsoft CRT archive digest mismatch")
    with zipfile.ZipFile(io.BytesIO(archive)) as source:
        member = source.getinfo(pin["member"])
        if member.file_size > 1024 * 1024:
            raise ValueError("CRT member exceeds size limit")
        data = source.read(member)
    if hashlib.sha256(data).hexdigest() != pin["dllSha256"]:
        raise ValueError("Microsoft CRT DLL digest mismatch")
    # Extract exactly one retail member, never debug_nonredist or installer files.
    with (root / "bin/vcruntime140.dll").open("xb") as output:
        output.write(data)
    files = list((root / "bin").glob("*.dll"))
    bundled = {path.name.lower() for path in files}
    imports = set()
    for path in files:
        if path.name.lower() != "vcruntime140.dll":
            imports.update(inspect(path, target).imports)
    # Only Microsoft's hash-pinned CRT may use ARM64X rather than plain ARM64.
    for path in (helper, root / "bin/vcruntime140.dll"):
        result = subprocess.run(
            ["dumpbin", "/nologo", "/dependents", str(path)],
            check=True,
            capture_output=True,
            timeout=30,
        )
        imports.update(
            re.findall(
                r"(?mi)^ +([a-z0-9_+.-]+\.dll)\s*$", result.stdout.decode("ascii")
            )
        )
    # Additional Windows OS imports used by the Rust helper, not bundled CRTs.
    system = (EXTERNAL_IMPORTS - {"vcruntime140.dll"}) | {
        "api-ms-win-core-synch-l1-2-0.dll",
        "api-ms-win-core-winrt-error-l1-1-0.dll",
        "bcrypt.dll",
        "bcryptprimitives.dll",
        "combase.dll",
        "mmdevapi.dll",
        "oleaut32.dll",
        "ntdll.dll",
        "userenv.dll",
        "dbghelp.dll",
    }
    missing = {name.lower() for name in imports} - bundled - system
    if missing:
        raise ValueError(f"Unbundled Windows imports: {sorted(missing)}")
    manifest_path = root / "runtime.json"
    manifest = json.loads(manifest_path.read_text())
    if manifest.get("target") != target or manifest.get("developmentOnly") is not True:
        raise ValueError("expected staged development runtime")
    manifest["libraries"].append(
        {"path": "bin/vcruntime140.dll", "sha256": pin["dllSha256"]}
    )
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--helper", type=Path, required=True)
    args = parser.parse_args()
    stage(args.root, args.target, args.helper)
