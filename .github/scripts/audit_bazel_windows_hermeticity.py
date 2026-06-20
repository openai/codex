#!/usr/bin/env python3
"""Fail-closed policy audit for Windows Bazel build actions."""

import json
import os
import posixpath
import re
import sys
from collections.abc import Callable, Iterable, Mapping
from pathlib import Path, PurePosixPath


REPO_ROOT = Path(__file__).resolve().parents[2]

_REQUIRED_MNEMONICS = {
    "ArgumentCommentLint",
    "CargoBuildScriptRun",
    "CppArchive",
    "CppCompile",
    "CppLink",
    "Rustc",
    "V8Mksnapshot",
}
_REQUIRED_TARGET_MARKERS = {
    "aws-lc-sys": "crates__aws-lc-sys-",
    "ring": "crates__ring-",
    "stack protector probe": "//tools/windows-toolchain:stack-protector-probe",
    "V8 consumer": "//codex-rs/v8-poc:v8-poc",
    "zstd-sys": "crates__zstd-sys-",
}
_FORBIDDEN_ENVIRONMENT = {
    "BAZEL_LLVM",
    "BAZEL_LLVM_COV",
    "BAZEL_LLVM_PROFDATA",
    "BAZEL_SH",
    "BAZEL_VC",
    "BAZEL_VC_FULL_VERSION",
    "BAZEL_VS",
    "BAZEL_WINSDK_FULL_VERSION",
    "CL",
    "DEVENVDIR",
    "EXTENSIONSDKDIR",
    "FRAMEWORKDIR",
    "FRAMEWORKDIR32",
    "FRAMEWORKVERSION",
    "FRAMEWORKVERSION32",
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "LINK",
    "NETFXSDKDIR",
    "UCRTVERSION",
    "UNIVERSALCRTSDKDIR",
    "USE_CLANG_CL",
    "VCINSTALLDIR",
    "VCIDEINSTALLDIR",
    "VCTOOLSINSTALLDIR",
    "VCTOOLSREDISTDIR",
    "VISUALSTUDIOVERSION",
    "VSCMD_ARG_HOST_ARCH",
    "VSCMD_ARG_TGT_ARCH",
    "VSCMD_VER",
    "VSCMD_START_DIR",
    "VSINSTALLDIR",
    "WINDOWSSDKBINPATH",
    "WINDOWSLIBPATH",
    "WINDOWSSDKDIR",
    "WINDOWSSDKLIBVERSION",
    "WINDOWSSDKVERSION",
    "WINDOWSSDKVERBINPATH",
    "_CL_",
    "_LINK_",
}
_TOOL_ENVIRONMENT_SUFFIXES = {
    "AR",
    "AS",
    "CARGO",
    "CC",
    "CMAKE",
    "CXX",
    "CXXFILT",
    "DLLTOOL",
    "GO",
    "LD",
    "MAKE",
    "MESON",
    "NASM",
    "NINJA",
    "NM",
    "NODE",
    "OBJCOPY",
    "OBJDUMP",
    "PERL",
    "PKG_CONFIG",
    "PROTOC",
    "PYTHON",
    "PYTHON3",
    "RANLIB",
    "RC",
    "RUSTC",
    "RUSTDOC",
    "STRIP",
    "WINDRES",
    "YASM",
}
_TARGET_TOOL_ENVIRONMENT = {
    "AR",
    "AS",
    "CC",
    "CXX",
    "DLLTOOL",
    "LD",
    "NM",
    "OBJCOPY",
    "OBJDUMP",
    "RANLIB",
    "RC",
    "STRIP",
    "WINDRES",
}
_FORBIDDEN_HOST_PATHS = (
    re.compile(r"(?:^|/)(?:program files(?: \(x86\))?/)?microsoft visual studio/"),
    re.compile(r"(?:^|/)windows kits/"),
    re.compile(r"(?:^|/)microsoft sdks/"),
    re.compile(r"(?:^|/)program files(?: \(x86\))?/llvm/"),
    re.compile(r"(?:^|[^a-z0-9_])[a-z]:/llvm/(?:bin|lib)/"),
    re.compile(r"(?:^|/)(?:msys64|mingw32|mingw64)/(?:[^/]*/)?bin/"),
    re.compile(
        r"(?:^|/)(?:usr|opt)/(?:[^/]+/)*bin/"
        r"(?:ar|as|cc|clang(?:\+\+|-cl)?|cmake|g\+\+|gcc|go|ld(?:\.lld)?|"
        r"lld-link|llvm-ar|make|nasm|ninja|node|perl|protoc|python(?:3)?|"
        r"ranlib|windres)(?:\.exe)?(?:$|[\s;])"
    ),
    re.compile(
        r"(?:^|/)(?:hostedtoolcache|_work/_tool)/(?:windows/)?(?:python|node(?:js)?)/"
    ),
    re.compile(r"(?:^|/)chocolatey/(?:bin|lib)/(?:llvm|mingw|python|node)"),
)
_BARE_COMPILER_COMMAND = re.compile(
    r"(?:^|[\s;&|()=\"'])"
    r"(?:ar|as|c\+\+|cargo|cc|clang(?:\+\+|-cl)?|cmake|dlltool|g\+\+|gcc|go|"
    r"ld(?:\.lld)?|link|lld-link|llvm-ar|make|meson|nasm|ninja|nm|node|objcopy|"
    r"perl|protoc|python(?:3)?|ranlib|rustc|rustdoc|strip|windres|yasm)"
    r"(?:\.exe)?(?=$|[\s;&|()\"'])",
    re.IGNORECASE,
)
_NESTED_TOOL_PREFIXES = (
    "--ar=",
    "--compiler=",
    "--linker=",
    "--node=",
    "--python=",
    "--script=",
    "-clinker=",
)


class AuditFailure(RuntimeError):
    pass


def _normalize(value: str) -> str:
    value = value.strip().strip('"').replace("\\", "/")
    for prefix in ("${pwd}/", "${exec_root}/"):
        if value.lower().startswith(prefix):
            value = value[len(prefix) :]
            break
    while value.startswith("./"):
        value = value[2:]
    return posixpath.normpath(value).lower()


def _path_entries(value: str) -> list[str]:
    if ";" not in value:
        return [_normalize(entry) for entry in value.split(":") if entry]

    entries: list[str] = []
    for entry in value.split(";"):
        if re.match(r"^[A-Za-z]:[\\/]", entry) or ":" not in entry:
            entries.append(_normalize(entry))
        else:
            entries.extend(_normalize(item) for item in entry.split(":") if item)
    return [entry for entry in entries if entry]


def _expected_windows_paths(
    environment: Mapping[str, str],
) -> tuple[list[str], list[str]] | None:
    program_files = environment.get("ProgramFiles")
    local_app_data = environment.get("LOCALAPPDATA")
    windows_dir = environment.get("WINDIR") or environment.get("SystemRoot")
    if not program_files or not local_app_data or not windows_dir:
        return None

    git_root = f"{program_files}/Git"
    execution = [
        f"{git_root}/usr/bin",
        f"{windows_dir}/System32",
        windows_dir,
    ]
    test = [
        f"{program_files}/PowerShell/7",
        f"{git_root}/bin",
        f"{git_root}/usr/bin",
        f"{local_app_data}/Microsoft/WindowsApps",
        f"{windows_dir}/System32/WindowsPowerShell/v1.0",
        f"{windows_dir}/System32",
        windows_dir,
    ]
    return (
        [_normalize(entry) for entry in execution],
        [_normalize(entry) for entry in test],
    )


def _is_tool_environment(key: str) -> bool:
    upper = key.upper()
    if upper.startswith("CARGO_TARGET_") and upper.endswith("_LINKER"):
        return True
    if upper.startswith(("CARGO_CFG_", "CARGO_FEATURE_")):
        return False
    tokens = upper.split("_")
    return (
        upper in _TOOL_ENVIRONMENT_SUFFIXES
        or (tokens[0] in _TARGET_TOOL_ENVIRONMENT and len(tokens) > 1)
        or (tokens[-1] in _TARGET_TOOL_ENVIRONMENT and len(tokens) > 1)
        or (tokens[0] == "LLVM" and tokens[-1] in _TOOL_ENVIRONMENT_SUFFIXES)
    )


def _artifact_paths(data: Mapping[str, object]) -> dict[int, str]:
    fragments = {int(item["id"]): item for item in data["pathFragments"]}  # type: ignore[index]

    def fragment_path(fragment_id: int) -> str:
        labels: list[str] = []
        while fragment_id:
            fragment = fragments[fragment_id]
            labels.append(str(fragment["label"]))
            fragment_id = int(fragment.get("parentId", 0))
        return "/".join(reversed(labels))

    return {
        int(artifact["id"]): _normalize(fragment_path(int(artifact["pathFragmentId"])))
        for artifact in data["artifacts"]  # type: ignore[index]
    }


def _action_input_resolver(
    data: Mapping[str, object], artifacts: Mapping[int, str]
) -> Callable[[Mapping[str, object]], set[str]]:
    dep_sets = {int(item["id"]): item for item in data["depSetOfFiles"]}  # type: ignore[index]
    memo: dict[int, set[int]] = {}

    def expand(dep_set_id: int) -> set[int]:
        if dep_set_id in memo:
            return memo[dep_set_id]
        dep_set = dep_sets[dep_set_id]
        artifact_ids = {int(item) for item in dep_set.get("directArtifactIds", [])}
        for transitive_id in dep_set.get("transitiveDepSetIds", []):
            artifact_ids.update(expand(int(transitive_id)))
        memo[dep_set_id] = artifact_ids
        return artifact_ids

    def resolve(action: Mapping[str, object]) -> set[str]:
        artifact_ids: set[int] = set()
        for dep_set_id in action.get("inputDepSetIds", []):
            artifact_ids.update(expand(int(dep_set_id)))
        return {artifacts[artifact_id] for artifact_id in artifact_ids}

    return resolve


def _is_declared_tool(tool: str, inputs: set[str]) -> bool:
    return _normalize(tool) in inputs


def _allowed_substrate_tools(environment: Mapping[str, str], mnemonic: str) -> set[str]:
    allowed = {
        "/bin/bash",
        "/usr/bin/bash",
        "bash",
        "bash.exe",
        "cmd.exe",
        "sh",
        "sh.exe",
    }
    paths = _expected_windows_paths(environment)
    if paths:
        program_files = environment["ProgramFiles"]
        windows_dir = environment.get("WINDIR") or environment["SystemRoot"]
        local_app_data = environment["LOCALAPPDATA"]
        allowed.update(
            {
                f"{program_files}/Git/usr/bin/bash.exe",
                f"{program_files}/Git/usr/bin/sh.exe",
                f"{windows_dir}/System32/cmd.exe",
            }
        )
        if mnemonic == "TestRunner":
            allowed.update(
                {
                    f"{program_files}/PowerShell/7/pwsh.exe",
                    f"{local_app_data}/Microsoft/WindowsApps/dotslash.exe",
                    f"{windows_dir}/System32/WindowsPowerShell/v1.0/powershell.exe",
                }
            )
    return {_normalize(item) for item in allowed}


def _tool_candidates(
    arguments: list[str],
    env: Mapping[str, str],
    *,
    include_action_executable: bool = True,
) -> Iterable[tuple[str, str]]:
    if arguments and include_action_executable:
        yield ("action executable", arguments[0])
    for index, argument in enumerate(arguments):
        lower = argument.lower()
        if argument == "--" and index + 1 < len(arguments):
            yield ("wrapped executable", arguments[index + 1])
        for prefix in _NESTED_TOOL_PREFIXES:
            if lower.startswith(prefix):
                yield (prefix.rstrip("="), argument[len(prefix) :])
        if lower.startswith("--codegen=linker="):
            yield ("rust linker", argument[len("--codegen=linker=") :])
        if lower.startswith("-c") and "linker=" in lower:
            yield ("rust linker", argument.split("linker=", 1)[1])

    for key, value in env.items():
        if value and _is_tool_environment(key):
            yield (f"environment {key}", value.split()[0])
        elif value and re.search(r"(?:^|[/\\])[^ ]+\.exe$", value, re.IGNORECASE):
            yield (f"executable-valued environment {key}", value)


def _is_windows_gnullvm_platform(value: str) -> bool:
    normalized = _normalize(value)
    return normalized.endswith(("//:local_windows", "//:windows_x86_64_gnullvm"))


def _audit_path(
    value: str,
    mnemonic: str,
    inputs: set[str],
    environment: Mapping[str, str],
) -> str | None:
    entries = _path_entries(value)
    expected = _expected_windows_paths(environment)
    build_paths = [["/usr/bin", "/bin"]]
    test_paths: list[list[str]] = []
    if expected:
        build_paths.append(expected[0])
        test_paths.append(expected[1])

    allowed_suffixes = (
        [*test_paths, *build_paths] if mnemonic == "TestRunner" else build_paths
    )
    input_directories = {_normalize(str(PurePosixPath(item).parent)) for item in inputs}
    for suffix in allowed_suffixes:
        if len(entries) < len(suffix) or entries[-len(suffix) :] != suffix:
            continue
        prefix = entries[: -len(suffix)]
        if not prefix or (
            mnemonic == "ArgumentCommentLint"
            and all(entry in input_directories for entry in prefix)
        ):
            return None
    return f"{mnemonic} has non-frozen PATH: {value}"


def _forbidden_value(value: str) -> bool:
    normalized = _normalize(value)
    return any(pattern.search(normalized) for pattern in _FORBIDDEN_HOST_PATHS)


def audit_aquery_files(
    paths: Iterable[Path], environment: Mapping[str, str] | None = None
) -> None:
    if environment is None:
        environment = os.environ
    errors: list[str] = []
    mnemonics: set[str] = set()
    target_labels: set[str] = set()
    saw_windows_build_script = False
    saw_windows_lint = False
    saw_windows_proc_macro = False
    probe_compile = False
    probe_link = False
    probe_test = False
    action_count = 0

    for path in paths:
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
            artifacts = _artifact_paths(data)
            resolve_inputs = _action_input_resolver(data, artifacts)
        except (KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
            raise AuditFailure(f"invalid aquery JSON in {path}: {error}") from error

        target_by_id = {
            int(target["id"]): str(target["label"])
            for target in data.get("targets", [])
        }
        target_labels.update(target_by_id.values())
        for action in data["actions"]:
            action_count += 1
            mnemonic = str(action.get("mnemonic", ""))
            mnemonics.add(mnemonic)
            arguments = [str(argument) for argument in action.get("arguments", [])]
            param_arguments = [
                str(argument)
                for param_file in action.get("paramFiles", [])
                for argument in param_file.get("arguments", [])
            ]
            all_arguments = [*arguments, *param_arguments]
            embedded_contents = [
                str(action.get("templateContent", "")),
                str(action.get("fileContents", "")),
                *[
                    str(substitution.get("value", ""))
                    for substitution in action.get("substitutions", [])
                ],
            ]
            env = {
                str(item["key"]): str(item.get("value", ""))
                for item in action.get("environmentVariables", [])
            }
            inputs = resolve_inputs(action)
            identity = f"{path.name}:{mnemonic or '<unknown>'}"
            target_label = target_by_id.get(int(action.get("targetId", 0)), "")
            execution_platform = str(action.get("executionPlatform", ""))
            windows_gnullvm_exec = _is_windows_gnullvm_platform(execution_platform)

            if any("--crate-type=proc-macro" == argument for argument in all_arguments):
                if (
                    windows_gnullvm_exec
                    and "--target=x86_64-pc-windows-gnullvm" in all_arguments
                ):
                    saw_windows_proc_macro = True
            if mnemonic == "CargoBuildScriptRun" and windows_gnullvm_exec:
                if (
                    env.get("HOST") == "x86_64-pc-windows-gnullvm"
                    and env.get("TARGET") == "x86_64-pc-windows-gnullvm"
                ):
                    saw_windows_build_script = True
            if mnemonic == "ArgumentCommentLint" and windows_gnullvm_exec:
                has_driver = any(
                    "argument-comment-lint-driver" in item
                    for item in [*all_arguments, *inputs]
                )
                has_rustc_driver = any("rustc_driver" in item for item in inputs)
                if (
                    has_driver
                    and has_rustc_driver
                    and "--target=x86_64-pc-windows-gnullvm" in all_arguments
                ):
                    saw_windows_lint = True

            for key in env:
                if key.upper() in _FORBIDDEN_ENVIRONMENT:
                    errors.append(
                        f"{identity} forwards forbidden environment variable {key}"
                    )

            for key, value in env.items():
                if key.upper() == "PATH":
                    if path_error := _audit_path(value, mnemonic, inputs, environment):
                        errors.append(f"{identity}: {path_error}")
                if _forbidden_value(value):
                    errors.append(
                        f"{identity} contains forbidden host path in {key}: {value}"
                    )
            for argument in all_arguments:
                if _forbidden_value(argument):
                    errors.append(
                        f"{identity} contains forbidden host path: {argument}"
                    )
                if argument.lower().startswith("bash_bin_path="):
                    expected_paths = _expected_windows_paths(environment)
                    allowed_bash_paths = {"bash_bin_path=/bin/bash"}
                    if expected_paths:
                        program_files = environment["ProgramFiles"].replace("\\", "/")
                        allowed_bash_paths.add(
                            f"bash_bin_path={program_files}/Git/usr/bin/bash.exe".lower()
                        )
                    if argument.lower().replace("\\", "/") not in allowed_bash_paths:
                        errors.append(
                            f"{identity} uses non-frozen Bazel shell: {argument}"
                        )
            for content in embedded_contents:
                if not content:
                    continue
                if _forbidden_value(content):
                    errors.append(f"{identity} writes a forbidden host path")
                if re.search(
                    r"(?:--host|--target)[= ]\S*windows-msvc",
                    content,
                    re.IGNORECASE,
                ):
                    errors.append(f"{identity} writes an MSVC target command")
                if content.lstrip().startswith(("#!", "set -")):
                    if match := _BARE_COMPILER_COMMAND.search(content):
                        errors.append(
                            f"{identity} writes a script that resolves a tool from PATH: "
                            f"{match.group(0).strip()}"
                        )

            execution_platform_lower = execution_platform.lower()
            msvc_target = any(
                argument.lower().startswith(("--host=", "--target="))
                and "windows-msvc" in argument.lower()
                for argument in all_arguments
            ) or any(
                key.upper() in {"CARGO_BUILD_TARGET", "HOST", "TARGET"}
                and "windows-msvc" in value.lower()
                for key, value in env.items()
            )
            msvc_target_env = env.get("CARGO_CFG_TARGET_ENV", "").lower() == "msvc"
            if (
                (
                    "windows" in execution_platform_lower
                    and "msvc" in execution_platform_lower
                )
                or msvc_target
                or msvc_target_env
            ):
                errors.append(f"{identity} selects an MSVC Windows platform or target")

            tool_candidates = [
                *_tool_candidates(arguments, env),
                *_tool_candidates(
                    param_arguments,
                    {},
                    include_action_executable=False,
                ),
            ]
            for description, tool in tool_candidates:
                normalized = _normalize(tool)
                if not normalized:
                    continue
                if "windows-msvc" in normalized or "windows_msvc" in normalized:
                    errors.append(f"{identity} selects an MSVC {description}: {tool}")
                    continue
                if _is_declared_tool(normalized, inputs):
                    continue
                if normalized in _allowed_substrate_tools(environment, mnemonic):
                    continue
                errors.append(f"{identity} uses undeclared {description}: {tool}")

            if arguments and _normalize(arguments[0]) in _allowed_substrate_tools(
                environment, mnemonic
            ):
                shell_command = " ".join(arguments[1:])
                if match := _BARE_COMPILER_COMMAND.search(shell_command):
                    errors.append(
                        f"{identity} resolves undeclared tool from PATH: {match.group(0).strip()}"
                    )

            if target_label == "//tools/windows-toolchain:stack-protector-probe":
                if mnemonic == "CppCompile":
                    required = {
                        "-fstack-protector-all",
                        "--sysroot=/dev/null",
                        "x86_64-w64-windows-gnu",
                    }
                    missing = required - set(all_arguments)
                    if missing:
                        errors.append(
                            f"{identity} stack-protector compile is missing: {sorted(missing)}"
                        )
                    else:
                        probe_compile = True
                elif mnemonic == "CppLink":
                    required = {
                        "-fuse-ld=lld",
                        "--sysroot=/dev/null",
                        "x86_64-w64-windows-gnu",
                    }
                    required_inputs = {
                        "crt_objects_directory_windows",
                        "mingw_crt_library_search_directory",
                        "mingw_import_libraries_directory",
                    }
                    missing = required - set(all_arguments)
                    missing_inputs = {
                        marker
                        for marker in required_inputs
                        if not any(marker in input_path for input_path in inputs)
                    }
                    if missing or missing_inputs:
                        errors.append(
                            f"{identity} stack-protector link is missing flags {sorted(missing)} "
                            f"or declared runtime inputs {sorted(missing_inputs)}"
                        )
                    else:
                        probe_link = True
                elif mnemonic == "TestRunner":
                    has_probe_runfiles = any(
                        input_path.endswith("/stack-protector-probe.exe.runfiles")
                        for input_path in inputs
                    )
                    has_probe_executable = (
                        len(arguments) > 1
                        and arguments[1]
                        == "tools/windows-toolchain/stack-protector-probe.exe"
                    )
                    if (
                        not _is_windows_gnullvm_platform(execution_platform)
                        or not has_probe_runfiles
                        or not has_probe_executable
                    ):
                        errors.append(
                            f"{identity} stack-protector test lacks the fixed Windows gnullvm runfiles"
                        )
                    else:
                        probe_test = True

    if action_count == 0:
        errors.append("aquery audit received no actions")
    for mnemonic in sorted(_REQUIRED_MNEMONICS - mnemonics):
        errors.append(f"aquery coverage is missing {mnemonic} actions")
    if not saw_windows_build_script:
        errors.append(
            "aquery coverage is missing a Windows-gnullvm build-script runner"
        )
    if not saw_windows_lint:
        errors.append(
            "aquery coverage is missing the Windows-gnullvm argument-comment lint driver"
        )
    if not saw_windows_proc_macro:
        errors.append(
            "aquery coverage is missing a Windows-gnullvm proc-macro Rustc action"
        )
    if not probe_compile:
        errors.append("aquery coverage is missing the protected C compile probe")
    if not probe_link:
        errors.append("aquery coverage is missing the protected C link probe")
    if not probe_test:
        errors.append("aquery coverage is missing the protected C Windows run probe")
    for description, marker in _REQUIRED_TARGET_MARKERS.items():
        if not any(marker in label for label in target_labels):
            errors.append(f"aquery coverage is missing {description}")

    if errors:
        raise AuditFailure(
            "Windows Bazel hermeticity audit failed:\n- " + "\n- ".join(errors)
        )


def _repo_files(repo_root: Path) -> Iterable[Path]:
    for directory, directory_names, file_names in os.walk(repo_root):
        directory_names[:] = [
            name
            for name in directory_names
            if name
            not in {
                ".cache",
                ".git",
                "D:",
                "__pycache__",
                "bazel-out",
                "node_modules",
                "target",
            }
            and not name.startswith("bazel-")
        ]
        for file_name in file_names:
            yield Path(directory) / file_name


def _source_files(repo_root: Path) -> Iterable[Path]:
    for path in _repo_files(repo_root):
        relative = path.relative_to(repo_root)
        if path.name in {
            "BUILD",
            "BUILD.bazel",
            "MODULE.bazel",
            "WORKSPACE",
            "WORKSPACE.bazel",
        }:
            yield path
        elif path.suffix == ".bzl" or relative == Path(".bazelrc"):
            yield path
        elif relative.parts[:2] == (".github", "workflows") and path.suffix in {
            ".yaml",
            ".yml",
        }:
            yield path
        elif relative.parts[:1] == ("patches",) and path.suffix == ".patch":
            yield path
        elif relative.parts[:2] == (".github", "actions") and any(
            marker in part
            for marker in ("argument-comment-lint", "bazel")
            for part in relative.parts[2:]
        ):
            yield path
        elif relative.parts[:2] == (".github", "scripts") and "bazel" in path.name:
            if not path.name.startswith("test_"):
                yield path
        elif relative.parts[:1] in {("scripts",), ("tools",)} and "bazel" in path.name:
            yield path


def audit_source_tree(repo_root: Path = REPO_ROOT) -> None:
    errors: list[str] = []
    self_paths = {
        Path(".github/scripts/audit_bazel_windows_hermeticity.py"),
        Path(".github/scripts/test_audit_bazel_windows_hermeticity.py"),
    }
    action_env_allowlist = {
        '"--action_env=PATH=${windows_execution_path}"',
        '"--host_action_env=PATH=${windows_execution_path}"',
    }
    repo_env_allowlist = {
        "common --repo_env=BAZEL_DO_NOT_DETECT_CPP_TOOLCHAIN=1",
        "common --repo_env=BAZEL_NO_APPLE_CPP_TOOLCHAIN=1",
    }
    client_path_removal_allowlist = {
        '*"/microsoft visual studio/"* | \\',
        '*"/windows kits/"* | \\',
        '*"/microsoft sdks/"* | \\',
        '*"/program files/llvm/"* | \\',
        '*"/program files (x86)/llvm/"* | \\',
        '*"/msys64/"* | \\',
        '*"/mingw32/"* | \\',
        '*"/mingw64/"*)',
    }
    local_repository_occurrences: list[tuple[Path, str]] = []
    repository_rule_occurrences: list[tuple[Path, str]] = []

    for path in _source_files(repo_root):
        relative = path.relative_to(repo_root)
        if relative in self_paths:
            continue
        for line_number, raw_line in enumerate(
            path.read_text(encoding="utf-8", errors="replace").splitlines(), 1
        ):
            if (
                path.suffix == ".patch"
                and raw_line.startswith("-")
                and not raw_line.startswith("---")
            ):
                continue
            line = (
                raw_line[1:]
                if path.suffix == ".patch" and raw_line.startswith("+")
                else raw_line
            )
            lower = line.lower()
            location = f"{relative}:{line_number}"

            if "skip_incompatible_explicit_targets" in lower:
                errors.append(
                    f"{location} skips explicitly requested incompatible targets"
                )
            if re.search(r"\b[A-Za-z_][A-Za-z0-9_]*\.which\s*\(", line):
                errors.append(f"{location} discovers a tool from the host")
            if _forbidden_value(line) and not (
                relative
                == Path(".github/scripts/sanitize-bazel-windows-environment.sh")
                and line.strip() in client_path_removal_allowlist
            ):
                errors.append(f"{location} hard-codes a forbidden host toolchain path")
            if re.search(r"--(?:host_)?action_env=", line):
                if (
                    relative != Path(".github/scripts/run-bazel-ci.sh")
                    or line.strip() not in action_env_allowlist
                ):
                    errors.append(
                        f"{location} forwards a new build-action environment variable"
                    )
            if "--repo_env=" in line and (
                relative != Path(".bazelrc") or line.strip() not in repo_env_allowlist
            ):
                errors.append(
                    f"{location} forwards a new repository environment variable"
                )
            if "file://" in lower:
                errors.append(f"{location} imports an unpinned local file")
            if re.search(
                r"\b(?:which|where(?:\.exe)?|get-command)\s+(?:clang|gcc|cl|link|lld|nasm|cmake|ninja|python|node)\b",
                lower,
            ):
                errors.append(f"{location} discovers a compiler or native-build tool")
            if re.search(
                r"\b(?:new_local_repository|local_repository|local_path_override)\b",
                line,
            ):
                local_repository_occurrences.append((relative, line.strip()))
            if "repository_rule(" in line:
                repository_rule_occurrences.append((relative, line.strip()))

    expected_local_repository_occurrences = [
        (
            Path("MODULE.bazel"),
            'new_local_repository = use_repo_rule("@bazel_tools//tools/build_defs/repo:local.bzl", "new_local_repository")',
        ),
        (Path("MODULE.bazel"), "new_local_repository("),
    ]
    if local_repository_occurrences != expected_local_repository_occurrences:
        errors.append(
            "local repository policy changed; only the checked-in third_party/v8 repository is allowed"
        )
    module_text = (repo_root / "MODULE.bazel").read_text(encoding="utf-8")
    if not re.search(
        r'new_local_repository\(\s*name = "v8_targets",\s*build_file = "//third_party/v8:BUILD\.bazel",\s*path = "third_party/v8",\s*\)',
        module_text,
    ):
        errors.append(
            "the v8_targets local repository must remain pinned to third_party/v8"
        )
    expected_repository_rule_occurrences = [
        (Path("rbe.bzl"), "rbe_platform_repository = repository_rule("),
    ]
    if repository_rule_occurrences != expected_repository_rule_occurrences:
        errors.append(
            "new custom repository rules are forbidden by Windows hermeticity policy"
        )

    sanitizer_source = 'source "${script_dir}/sanitize-bazel-windows-environment.sh"'
    sanitizer_call = "sanitize_bazel_windows_environment"
    sanitizer_path = Path(".github/scripts/sanitize-bazel-windows-environment.sh")
    sanitizer_text = (repo_root / sanitizer_path).read_text(encoding="utf-8")
    frozen_bazel_shell = r'export BAZEL_SH="${ProgramFiles//\\//}/Git/usr/bin/bash.exe"'
    if sanitizer_text.count(frozen_bazel_shell) != 1:
        errors.append(
            f"{sanitizer_path} must freeze BAZEL_SH to the Git workflow shell"
        )
    for wrapper in (
        Path(".github/scripts/run-bazel-ci.sh"),
        Path(".github/scripts/run-bazel-query-ci.sh"),
    ):
        wrapper_lines = (repo_root / wrapper).read_text(encoding="utf-8").splitlines()
        if (
            wrapper_lines.count(sanitizer_source) != 1
            or wrapper_lines.count(sanitizer_call) != 1
        ):
            errors.append(
                f"{wrapper} must apply the fixed Windows environment sanitizer once"
            )

    text_suffixes = {
        ".bazel",
        ".bzl",
        ".json",
        ".md",
        ".patch",
        ".ps1",
        ".py",
        ".rs",
        ".sh",
        ".toml",
        ".yaml",
        ".yml",
    }
    for path in _repo_files(repo_root):
        if (
            path.relative_to(repo_root) in self_paths
            or path.suffix not in text_suffixes
        ):
            continue
        for line_number, line in enumerate(
            path.read_text(encoding="utf-8", errors="replace").splitlines(), 1
        ):
            if "skip_incompatible_explicit_targets" in line.lower():
                errors.append(
                    f"{path.relative_to(repo_root)}:{line_number} skips explicitly requested incompatible targets"
                )

    compute_path = repo_root / ".github/scripts/compute-bazel-windows-path.ps1"
    allowed_environment_roots = {
        "github_env",
        "localappdata",
        "programfiles",
        "systemroot",
        "windir",
    }
    for line_number, line in enumerate(
        compute_path.read_text(encoding="utf-8").splitlines(), 1
    ):
        for name in re.findall(r"\$env:([A-Za-z0-9_]+(?:\(x86\))?)", line):
            if name.lower() not in allowed_environment_roots:
                errors.append(
                    f"{compute_path.relative_to(repo_root)}:{line_number} extends the frozen path from {name}"
                )

    if errors:
        raise AuditFailure(
            "Windows Bazel source policy audit failed:\n- " + "\n- ".join(errors)
        )


def main(argv: list[str]) -> int:
    if not argv:
        print(
            f"usage: {Path(sys.argv[0]).name} <aquery.json> [<aquery.json> ...]",
            file=sys.stderr,
        )
        return 2
    try:
        audit_source_tree()
        audit_aquery_files(Path(argument) for argument in argv)
    except (AuditFailure, OSError) as error:
        print(error, file=sys.stderr)
        return 1
    print("Windows Bazel hermeticity audit passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
