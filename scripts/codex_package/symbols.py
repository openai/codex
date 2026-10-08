"""Strip packaged Unix executables without modifying their build outputs.

Call only on package copies of first-party binaries. Prebuilt release inputs
may already be signed, and resources such as bwrap have integrity hashes that
must survive staging. Windows MSVC symbols live in separate PDB files.
"""

import platform
import shutil
import subprocess
from pathlib import Path

from .targets import TargetSpec
from .targets import normalize_machine


def strip_binary(binary: Path, spec: TargetSpec, *, tool: str | None = None) -> None:
    if spec.is_windows:
        return

    if spec.is_linux:
        # A host GNU strip may not understand a cross-compiled ELF architecture.
        # LLVM supports both packaged Linux architectures from any host.
        machine = spec.target.split("-", 1)[0]
        command = (
            tool
            or shutil.which("llvm-strip")
            or shutil.which(f"{machine}-linux-gnu-strip")
        )
        if (
            command is None
            and platform.system() == "Linux"
            and normalize_machine(platform.machine()) == machine
        ):
            command = shutil.which("strip")
        if command is None:
            raise RuntimeError(
                f"No strip tool for {spec.target}; install llvm-strip or pass "
                "--strip-tool /path/to/target-strip (or --strip none to keep symbols)"
            )
        args = [command, "--strip-debug", "--strip-unneeded"]
    else:
        if tool is None and platform.system() != "Darwin":
            raise RuntimeError(
                "Stripping a macOS binary requires a Mach-O strip tool; pass "
                "--strip-tool /path/to/strip (or --strip none to keep symbols)"
            )
        # Match the release pipeline. Apple's strip also updates the ad-hoc
        # code signature on Cargo-built ARM64 executables so they remain runnable.
        args = [tool] if tool is not None else ["xcrun", "strip"]
        args.extend(("-S", "-x"))

    print(f"Stripping package executable {binary}")
    subprocess.run([*args, str(binary)], check=True)
