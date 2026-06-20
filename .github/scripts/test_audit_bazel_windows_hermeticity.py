#!/usr/bin/env python3

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

import audit_bazel_windows_hermeticity as audit


WINDOWS_ENV = {
    "CODEX_BAZEL_WINDOWS_EXECUTION_PATH": (
        r"C:\Program Files\Git\usr\bin;C:\Windows\System32;C:\Windows"
    ),
    "CODEX_BAZEL_WINDOWS_TEST_PATH": (
        r"C:\Program Files\PowerShell\7;C:\Program Files\Git\bin;"
        r"C:\Program Files\Git\usr\bin;C:\Users\runner\AppData\Local\Microsoft\WindowsApps;"
        r"C:\Windows\System32\WindowsPowerShell\v1.0;C:\Windows\System32;C:\Windows"
    ),
    "LOCALAPPDATA": r"C:\Users\runner\AppData\Local",
    "ProgramFiles": r"C:\Program Files",
    "WINDIR": r"C:\Windows",
}


def valid_aquery() -> dict[str, object]:
    artifact_paths = [
        "bazel-out/bin/process-wrapper.exe",
        "bazel-out/bin/rustc.exe",
        "bazel-out/bin/build-script-runner.exe",
        "bazel-out/bin/build-script.exe",
        "bazel-out/bin/argument-comment-lint-driver.exe",
        "bazel-out/bin/rustc_driver-nightly.dll",
        "external/llvm/bin/clang.exe",
        "external/llvm/bin/clang++.exe",
        "external/llvm/bin/llvm-ar.exe",
        "external/v8/mksnapshot.exe",
        "external/bazel_tools/tools/test/test-setup.sh",
        "bazel-out/windows/bin/tools/windows-toolchain/stack-protector-probe.exe.runfiles",
        "bazel-out/runtimes/crt_objects_directory_windows",
        "bazel-out/runtimes/mingw/mingw_crt_library_search_directory",
        "bazel-out/runtimes/mingw/mingw_import_libraries_directory",
    ]
    targets = [
        {"id": 1, "label": "//tools/windows-toolchain:stack-protector-probe"},
        {"id": 2, "label": "//codex-rs/v8-poc:v8-poc"},
        {
            "id": 3,
            "label": "@@rules_rs++crate+crates__aws-lc-sys-0.39.0//:aws-lc-sys",
        },
        {"id": 4, "label": "@@rules_rs++crate+crates__ring-0.17.14//:ring"},
        {
            "id": 5,
            "label": "@@rules_rs++crate+crates__zstd-sys-2.0.16+zstd.1.5.7//:zstd-sys",
        },
        {
            "id": 6,
            "label": "//codex-rs/codex-experimental-api-macros:codex-experimental-api-macros",
        },
    ]
    platform = "//:windows_x86_64_gnullvm"
    execution_path = WINDOWS_ENV["CODEX_BAZEL_WINDOWS_EXECUTION_PATH"]
    lint_path = rf"bazel-out/bin;{execution_path}"
    actions = [
        {
            "mnemonic": "Rustc",
            "targetId": 6,
            "executionPlatform": platform,
            "arguments": [
                "bazel-out/bin/process-wrapper.exe",
                "--",
                "bazel-out/bin/rustc.exe",
                "--crate-type=proc-macro",
                "--target=x86_64-pc-windows-gnullvm",
            ],
        },
        {
            "mnemonic": "CargoBuildScriptRun",
            "targetId": 3,
            "executionPlatform": platform,
            "arguments": [
                "bazel-out/bin/build-script-runner.exe",
                "--script=bazel-out/bin/build-script.exe",
            ],
            "environmentVariables": [
                {"key": "PATH", "value": execution_path},
                {"key": "HOST", "value": "x86_64-pc-windows-gnullvm"},
                {"key": "TARGET", "value": "x86_64-pc-windows-gnullvm"},
                {"key": "CC", "value": "external/llvm/bin/clang.exe"},
            ],
        },
        {
            "mnemonic": "ArgumentCommentLint",
            "targetId": 6,
            "executionPlatform": platform,
            "arguments": [
                "bazel-out/bin/process-wrapper.exe",
                "--",
                "bazel-out/bin/argument-comment-lint-driver.exe",
                "--target=x86_64-pc-windows-gnullvm",
            ],
            "environmentVariables": [{"key": "PATH", "value": lint_path}],
        },
        {
            "mnemonic": "CppCompile",
            "targetId": 1,
            "executionPlatform": platform,
            "arguments": [
                "external/llvm/bin/clang.exe",
                "-target",
                "x86_64-w64-windows-gnu",
                "--sysroot=/dev/null",
                "-fstack-protector-all",
            ],
        },
        {
            "mnemonic": "CppLink",
            "targetId": 1,
            "executionPlatform": platform,
            "arguments": [
                "external/llvm/bin/clang++.exe",
                "-target",
                "x86_64-w64-windows-gnu",
                "--sysroot=/dev/null",
                "-fuse-ld=lld",
            ],
        },
        {
            "mnemonic": "CppArchive",
            "targetId": 4,
            "executionPlatform": platform,
            "arguments": ["external/llvm/bin/llvm-ar.exe", "rcsD"],
        },
        {
            "mnemonic": "V8Mksnapshot",
            "targetId": 2,
            "executionPlatform": platform,
            "arguments": ["external/v8/mksnapshot.exe"],
        },
        {
            "mnemonic": "TestRunner",
            "targetId": 1,
            "executionPlatform": platform,
            "arguments": [
                "external/bazel_tools/tools/test/test-setup.sh",
                "tools/windows-toolchain/stack-protector-probe.exe",
            ],
            "environmentVariables": [
                {
                    "key": "PATH",
                    "value": WINDOWS_ENV["CODEX_BAZEL_WINDOWS_TEST_PATH"],
                }
            ],
        },
    ]
    for action in actions:
        action["inputDepSetIds"] = [1]
    return {
        "actions": actions,
        "artifacts": [
            {"id": index, "pathFragmentId": index}
            for index in range(1, len(artifact_paths) + 1)
        ],
        "depSetOfFiles": [
            {"id": 1, "directArtifactIds": list(range(1, len(artifact_paths) + 1))}
        ],
        "pathFragments": [
            {"id": index, "label": path} for index, path in enumerate(artifact_paths, 1)
        ],
        "targets": targets,
    }


class WindowsBazelHermeticityAuditTest(unittest.TestCase):
    def audit_graph(self, graph: dict[str, object]) -> None:
        with TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "aquery.json"
            path.write_text(json.dumps(graph), encoding="utf-8")
            audit.audit_aquery_files([path], WINDOWS_ENV)

    def assert_graph_fails(self, graph: dict[str, object], message: str) -> None:
        with self.assertRaisesRegex(audit.AuditFailure, message):
            self.audit_graph(graph)

    def test_accepts_declared_windows_gnullvm_toolchain(self) -> None:
        self.audit_graph(valid_aquery())

    def test_rejects_bare_tool_even_when_same_basename_is_declared(self) -> None:
        graph = valid_aquery()
        graph["actions"][5]["arguments"][0] = "llvm-ar.exe"  # type: ignore[index]
        self.assert_graph_fails(graph, "undeclared action executable")

    def test_rejects_forbidden_host_toolchain_path(self) -> None:
        graph = valid_aquery()
        graph["actions"][3]["arguments"][0] = (  # type: ignore[index]
            r"C:\Program Files\LLVM\bin\clang.exe"
        )
        self.assert_graph_fails(graph, "forbidden host path")

    def test_rejects_forbidden_environment_even_when_empty(self) -> None:
        graph = valid_aquery()
        graph["actions"][1]["environmentVariables"].append(  # type: ignore[index]
            {"key": "VCToolsInstallDir"}
        )
        self.assert_graph_fails(graph, "forbidden environment variable")

    def test_rejects_path_extension_for_build_actions(self) -> None:
        graph = valid_aquery()
        graph["actions"][1]["environmentVariables"][0]["value"] += (  # type: ignore[index]
            r";C:\extra\bin"
        )
        self.assert_graph_fails(graph, "non-frozen PATH")

    def test_accepts_declared_lint_directory_before_linux_substrate(self) -> None:
        graph = valid_aquery()
        graph["actions"][2]["environmentVariables"][0]["value"] = (  # type: ignore[index]
            "bazel-out/bin;/usr/bin:/bin"
        )
        self.audit_graph(graph)

    def test_rejects_forbidden_path_in_param_file(self) -> None:
        graph = valid_aquery()
        graph["actions"][3]["paramFiles"] = [  # type: ignore[index]
            {
                "execPath": "bazel-out/compile.params",
                "arguments": [r"-IC:\Program Files\LLVM\include"],
            }
        ]
        self.assert_graph_fails(graph, "forbidden host path")

    def test_rejects_undeclared_linker_in_param_file(self) -> None:
        graph = valid_aquery()
        graph["actions"][0]["paramFiles"] = [  # type: ignore[index]
            {
                "execPath": "bazel-out/rustc.params",
                "arguments": ["--codegen=linker=clang.exe"],
            }
        ]
        self.assert_graph_fails(graph, "undeclared rust linker")

    def test_rejects_msvc_execution_platform(self) -> None:
        graph = valid_aquery()
        graph["actions"][5]["executionPlatform"] = "//:windows_x86_64_msvc"  # type: ignore[index]
        self.assert_graph_fails(graph, "MSVC Windows platform")

    def test_rejects_missing_v8_action_coverage(self) -> None:
        graph = valid_aquery()
        graph["actions"] = [  # type: ignore[index]
            action
            for action in graph["actions"]  # type: ignore[union-attr]
            if action["mnemonic"] != "V8Mksnapshot"
        ]
        self.assert_graph_fails(graph, "missing V8Mksnapshot")

    def source_tree(self, temp_dir: str, build_contents: str = "") -> Path:
        root = Path(temp_dir)
        (root / ".github/scripts").mkdir(parents=True)
        (root / ".github/scripts/compute-bazel-windows-path.ps1").write_text(
            "$windowsDir = $env:WINDIR\n$programFiles = $env:ProgramFiles\n"
            "$localAppData = $env:LOCALAPPDATA\n$githubEnv = $env:GITHUB_ENV\n",
            encoding="utf-8",
        )
        for wrapper in ("run-bazel-ci.sh", "run-bazel-query-ci.sh"):
            (root / ".github/scripts" / wrapper).write_text(
                'source "${script_dir}/sanitize-bazel-windows-environment.sh"\n'
                "sanitize_bazel_windows_environment\n",
                encoding="utf-8",
            )
        (root / ".github/scripts/sanitize-bazel-windows-environment.sh").write_text(
            r'export BAZEL_SH="${ProgramFiles//\\//}/Git/usr/bin/bash.exe"' + "\n",
            encoding="utf-8",
        )
        (root / "BUILD.bazel").write_text(build_contents, encoding="utf-8")
        (root / "MODULE.bazel").write_text(
            'new_local_repository = use_repo_rule("@bazel_tools//tools/build_defs/repo:local.bzl", "new_local_repository")\n'
            "new_local_repository(\n"
            '    name = "v8_targets",\n'
            '    build_file = "//third_party/v8:BUILD.bazel",\n'
            '    path = "third_party/v8",\n'
            ")\n",
            encoding="utf-8",
        )
        (root / "rbe.bzl").write_text(
            "rbe_platform_repository = repository_rule(\n", encoding="utf-8"
        )
        return root

    def test_source_policy_accepts_fixed_policy(self) -> None:
        with TemporaryDirectory() as temp_dir:
            audit.audit_source_tree(self.source_tree(temp_dir))

    def test_source_policy_rejects_tool_discovery(self) -> None:
        with TemporaryDirectory() as temp_dir:
            root = self.source_tree(temp_dir, 'tool = rctx.which("clang.exe")\n')
            with self.assertRaisesRegex(audit.AuditFailure, "discovers a tool"):
                audit.audit_source_tree(root)

    def test_source_policy_rejects_target_skipping_anywhere(self) -> None:
        with TemporaryDirectory() as temp_dir:
            root = self.source_tree(temp_dir)
            (root / "notes.md").write_text(
                "--skip_incompatible_explicit_targets\n", encoding="utf-8"
            )
            with self.assertRaisesRegex(audit.AuditFailure, "incompatible targets"):
                audit.audit_source_tree(root)

    def test_source_policy_rejects_new_build_environment_forwarding(self) -> None:
        with TemporaryDirectory() as temp_dir:
            root = self.source_tree(temp_dir, 'args = ["--action_env=CC"]\n')
            with self.assertRaisesRegex(audit.AuditFailure, "build-action environment"):
                audit.audit_source_tree(root)

    def test_source_policy_rejects_new_repository_environment_forwarding(self) -> None:
        with TemporaryDirectory() as temp_dir:
            root = self.source_tree(temp_dir, 'args = ["--repo_env=CC"]\n')
            with self.assertRaisesRegex(audit.AuditFailure, "repository environment"):
                audit.audit_source_tree(root)


if __name__ == "__main__":
    unittest.main()
