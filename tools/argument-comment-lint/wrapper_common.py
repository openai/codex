#!/usr/bin/env python3

from __future__ import annotations

from dataclasses import dataclass
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from typing import MutableMapping, Sequence

STRICT_LINT = "uncommented-anonymous-literal-argument"
NOISE_LINT = "unknown_lints"
TOOLCHAIN_CHANNEL = "nightly-2025-09-18"

_TARGET_SELECTION_ARGS = {
    "--all-targets",
    "--lib",
    "--bins",
    "--tests",
    "--examples",
    "--benches",
    "--doc",
}
_TARGET_SELECTION_PREFIXES = ("--bin=", "--test=", "--example=", "--bench=")
_TARGET_SELECTION_WITH_VALUE = {"--bin", "--test", "--example", "--bench"}
_NIGHTLY_LIBRARY_PATTERN = re.compile(
    r"^(.+@nightly-[0-9]{4}-[0-9]{2}-[0-9]{2})-.+$"
)


@dataclass
class ParsedWrapperArgs:
    lint_args: list[str]
    cargo_args: list[str]
    has_manifest_path: bool = False
    has_package_selection: bool = False
    has_no_deps: bool = False
    has_library_selection: bool = False
    has_cargo_target_selection: bool = False
    has_fix: bool = False


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def parse_wrapper_args(argv: Sequence[str]) -> ParsedWrapperArgs:
    parsed = ParsedWrapperArgs(lint_args=[], cargo_args=[])
    after_separator = False
    expect_value: str | None = None

    for arg in argv:
        if after_separator:
            parsed.cargo_args.append(arg)
            if arg in _TARGET_SELECTION_ARGS or arg in _TARGET_SELECTION_WITH_VALUE:
                parsed.has_cargo_target_selection = True
            elif arg.startswith(_TARGET_SELECTION_PREFIXES):
                parsed.has_cargo_target_selection = True
            continue

        if arg == "--":
            after_separator = True
            continue

        parsed.lint_args.append(arg)

        if expect_value is not None:
            if expect_value == "manifest_path":
                parsed.has_manifest_path = True
            elif expect_value == "package_selection":
                parsed.has_package_selection = True
            elif expect_value == "library_selection":
                parsed.has_library_selection = True
            expect_value = None
            continue

        if arg == "--manifest-path":
            expect_value = "manifest_path"
        elif arg.startswith("--manifest-path="):
            parsed.has_manifest_path = True
        elif arg in {"-p", "--package"}:
            expect_value = "package_selection"
        elif arg.startswith("--package="):
            parsed.has_package_selection = True
        elif arg == "--fix":
            parsed.has_fix = True
        elif arg == "--workspace":
            parsed.has_package_selection = True
        elif arg == "--no-deps":
            parsed.has_no_deps = True
        elif arg in {"--lib", "--lib-path"}:
            expect_value = "library_selection"
        elif arg.startswith("--lib=") or arg.startswith("--lib-path="):
            parsed.has_library_selection = True

    return parsed


def build_final_args(parsed: ParsedWrapperArgs, manifest_path: Path) -> list[str]:
    final_args: list[str] = []
    cargo_args = list(parsed.cargo_args)

    if not parsed.has_manifest_path:
        final_args.extend(["--manifest-path", str(manifest_path)])
    if not parsed.has_package_selection and not parsed.has_manifest_path:
        final_args.append("--workspace")
    if not parsed.has_no_deps:
        final_args.append("--no-deps")
    if not parsed.has_fix and not parsed.has_cargo_target_selection:
        cargo_args.append("--all-targets")
    final_args.extend(parsed.lint_args)
    if cargo_args:
        final_args.extend(["--", *cargo_args])
    return final_args


def append_env_flag(env: MutableMapping[str, str], key: str, flag: str) -> None:
    value = env.get(key)
    if value is None or value == "":
        env[key] = flag
        return
    if flag not in value:
        env[key] = f"{value} {flag}"


def set_default_lint_env(env: MutableMapping[str, str]) -> None:
    append_env_flag(env, "DYLINT_RUSTFLAGS", f"-D {STRICT_LINT}")
    append_env_flag(env, "DYLINT_RUSTFLAGS", f"-A {NOISE_LINT}")
    if not env.get("CARGO_INCREMENTAL"):
        env["CARGO_INCREMENTAL"] = "0"


def die(message: str) -> "Never":
    print(message, file=sys.stderr)
    raise SystemExit(1)


def require_command(name: str, install_message: str | None = None) -> str:
    executable = shutil.which(name)
    if executable is None:
        if install_message is None:
            die(f"{name} is required but was not found on PATH.")
        die(install_message)
    return executable


def run_capture(args: Sequence[str], env: MutableMapping[str, str] | None = None) -> str:
    try:
        completed = subprocess.run(
            list(args),
            capture_output=True,
            check=True,
            env=None if env is None else dict(env),
            text=True,
        )
    except subprocess.CalledProcessError as error:
        command = shlex.join(str(part) for part in error.cmd)
        stderr = error.stderr.strip()
        stdout = error.stdout.strip()
        output = stderr or stdout
        if output:
            die(f"{command} failed:\n{output}")
        die(f"{command} failed with exit code {error.returncode}")
    return completed.stdout.strip()


def ensure_source_prerequisites(env: MutableMapping[str, str]) -> None:
    require_command(
        "cargo-dylint",
        "argument-comment-lint source wrapper requires cargo-dylint and dylint-link.\n"
        "Install them with:\n"
        "  cargo install --locked cargo-dylint dylint-link",
    )
    require_command(
        "dylint-link",
        "argument-comment-lint source wrapper requires cargo-dylint and dylint-link.\n"
        "Install them with:\n"
        "  cargo install --locked cargo-dylint dylint-link",
    )
    require_command(
        "rustup",
        "argument-comment-lint source wrapper requires rustup.\n"
        f"Install the {TOOLCHAIN_CHANNEL} toolchain with:\n"
        f"  rustup toolchain install {TOOLCHAIN_CHANNEL} \\\n"
        "    --component llvm-tools-preview \\\n"
        "    --component rustc-dev \\\n"
        "    --component rust-src",
    )
    toolchains = run_capture(["rustup", "toolchain", "list"], env=env)
    if not any(line.startswith(TOOLCHAIN_CHANNEL) for line in toolchains.splitlines()):
        die(
            "argument-comment-lint source wrapper requires the "
            f"{TOOLCHAIN_CHANNEL} toolchain with rustc-dev support.\n"
            "Install it with:\n"
            f"  rustup toolchain install {TOOLCHAIN_CHANNEL} \\\n"
            "    --component llvm-tools-preview \\\n"
            "    --component rustc-dev \\\n"
            "    --component rust-src"
        )


def prefer_rustup_shims(env: MutableMapping[str, str]) -> None:
    if env.get("CODEX_ARGUMENT_COMMENT_LINT_SKIP_RUSTUP_SHIMS") == "1":
        return

    rustup = shutil.which("rustup", path=env.get("PATH"))
    if rustup is None:
        return

    rustup_bin_dir = str(Path(rustup).resolve().parent)
    path_entries = [
        entry
        for entry in env.get("PATH", "").split(os.pathsep)
        if entry and entry != rustup_bin_dir
    ]
    env["PATH"] = os.pathsep.join([rustup_bin_dir, *path_entries])

    if not env.get("RUSTUP_HOME"):
        rustup_home = run_capture(["rustup", "show", "home"], env=env)
        if rustup_home:
            env["RUSTUP_HOME"] = rustup_home


def fetch_packaged_entrypoint(dotslash_manifest: Path, env: MutableMapping[str, str]) -> Path:
    require_command(
        "dotslash",
        "argument-comment-lint prebuilt wrapper requires dotslash.\n"
        "Install dotslash, or use:\n"
        "  ./tools/argument-comment-lint/run.py ...",
    )
    entrypoint = run_capture(["dotslash", "--", "fetch", str(dotslash_manifest)], env=env)
    return Path(entrypoint).resolve()


def find_packaged_cargo_dylint(package_entrypoint: Path) -> Path:
    bin_dir = package_entrypoint.parent
    cargo_dylint = bin_dir / "cargo-dylint"
    if not cargo_dylint.is_file():
        cargo_dylint = bin_dir / "cargo-dylint.exe"
    if not cargo_dylint.is_file():
        die(f"bundled cargo-dylint executable not found under {bin_dir}")
    return cargo_dylint


def normalize_packaged_library(package_entrypoint: Path) -> Path:
    library_dir = package_entrypoint.parent.parent / "lib"
    libraries = sorted(path for path in library_dir.glob("*@*") if path.is_file())
    if not libraries:
        die(f"no packaged Dylint library found in {library_dir}")
    if len(libraries) != 1:
        die(f"expected exactly one packaged Dylint library in {library_dir}")

    library_path = libraries[0]
    match = _NIGHTLY_LIBRARY_PATTERN.match(library_path.stem)
    if match is None:
        return library_path

    temp_dir = Path(tempfile.mkdtemp(prefix="argument-comment-lint."))
    normalized_library_path = temp_dir / f"{match.group(1)}{library_path.suffix}"
    shutil.copy2(library_path, normalized_library_path)
    return normalized_library_path


def exec_command(command: Sequence[str], env: MutableMapping[str, str]) -> "Never":
    try:
        completed = subprocess.run(list(command), env=dict(env), check=False)
    except FileNotFoundError:
        die(f"{command[0]} is required but was not found on PATH.")
    raise SystemExit(completed.returncode)
