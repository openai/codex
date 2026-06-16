"""Macros for cross-building Windows Rust binaries and testing them with Wine."""

load("@rules_rust//rust:defs.bzl", "rust_test")
load("//:defs.bzl", "WINDOWS_GNULLVM_RUSTC_LINK_FLAGS")
load("//bazel/rules/testing:foreign_platform_binary.bzl", "foreign_platform_binary")
load(":wine_runtime.bzl", "WINE_TEST_TARGET_COMPATIBLE_WITH", "wine_test_runtime")

def wine_rust_test(
        name,
        windows_binaries,
        host_binaries = {},
        data = [],
        size = "small",
        target_compatible_with = [],
        **kwargs):
    """Defines an x86-64 Linux Rust test with a pinned Wine runtime.

    Each `windows_binaries` executable is transitioned to GNU/LLVM Windows;
    every Rust dependency receives the repository's Windows linker flags while
    the test stays on x86-64 Linux. Its environment-variable contract is:

    * Each `host_binaries` and `windows_binaries` entry contributes
      `CARGO_BIN_EXE_<binary_name>` for its executable.
    * `CARGO_BIN_EXE_wine` and `CARGO_BIN_EXE_wineserver` identify Wine tools.
    * `CARGO_BIN_EXE_wine-runtime-marker` identifies a file whose parent is the
      Wine DLL directory to use as `WINEDLLPATH`.
    * `CARGO_BIN_EXE_pwsh` identifies the pinned PowerShell executable and
      `CARGO_BIN_EXE_pwsh-runtime-marker` identifies a file whose parent is the
      complete PowerShell runtime.

    These are Bazel runfile locations. Resolve binaries with
    `codex_utils_cargo_bin::cargo_bin`; `:wine_test_support` resolves the fixed
    runtime names and starts each process in an isolated prefix.

    Args:
      name: Name of the generated Linux `rust_test`.
      windows_binaries: Map from `CARGO_BIN_EXE_*` suffixes to Windows targets.
      host_binaries: Map from `CARGO_BIN_EXE_*` suffixes to Linux host targets.
      data: Additional runtime data for the Linux test.
      size: Bazel test size. Defaults to small.
      target_compatible_with: Additional compatibility constraints.
      **kwargs: Remaining attributes forwarded to `rust_test`.
    """
    binaries = dict(host_binaries)
    for index, binary_name in enumerate(sorted(windows_binaries.keys())):
        if binary_name in binaries:
            fail("Windows test binary name collides with host binary: {}".format(binary_name))
        transitioned_binary = name + "-windows-binary-" + str(index)
        foreign_platform_binary(
            name = transitioned_binary,
            binary = windows_binaries[binary_name],
            extra_rustc_flags = WINDOWS_GNULLVM_RUSTC_LINK_FLAGS,
            platform = "//:windows_x86_64_gnullvm",
            tags = ["manual"],
            target_compatible_with = [
                "@platforms//cpu:x86_64",
                "@platforms//os:linux",
            ],
            testonly = True,
            visibility = ["//visibility:private"],
        )
        binaries[binary_name] = ":" + transitioned_binary

    runtime = wine_test_runtime(binaries)
    rust_test(
        name = name,
        data = data + runtime.data,
        env = runtime.env,
        size = size,
        target_compatible_with = target_compatible_with + WINE_TEST_TARGET_COMPATIBLE_WITH,
        **kwargs
    )
