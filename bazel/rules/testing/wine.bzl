"""Macros for cross-building Windows Rust binaries and testing them with Wine."""

load("@rules_rust//rust:defs.bzl", "rust_test")
load("//:defs.bzl", "WINDOWS_GNULLVM_RUSTC_LINK_FLAGS")
load(":foreign_platform_binary.bzl", "foreign_platform_binary")

_WINE_RUNTIME_BINARIES = {
    "wine": "@wine_linux_x86_64//:wine",
    "wine-runtime-marker": "@wine_linux_x86_64//:runtime_marker",
    "wineserver": "@wine_linux_x86_64//:wineserver",
}

def wine_rust_test(
        name,
        windows_binaries,
        data = [],
        target_compatible_with = [],
        **kwargs):
    """Defines an x86-64 Linux Rust test with a pinned Wine runtime.

    Each `windows_binaries` executable is transitioned to GNU/LLVM Windows;
    every Rust dependency receives the repository's Windows linker flags while
    the test stays on x86-64 Linux. Its environment-variable contract is:

    * Each entry contributes `CARGO_BIN_EXE_<binary_name>` for its executable.
    * `CARGO_BIN_EXE_wine` and `CARGO_BIN_EXE_wineserver` identify Wine tools.
    * `CARGO_BIN_EXE_wine-runtime-marker` identifies a file whose parent is the
      Wine DLL directory to use as `WINEDLLPATH`.

    These are Bazel runfile locations. Resolve binaries with
    `codex_utils_cargo_bin::cargo_bin`; `:wine_test_support` resolves the fixed
    runtime names and starts each process in an isolated prefix.

    Args:
      name: Name of the generated Linux `rust_test`.
      windows_binaries: Map from `CARGO_BIN_EXE_*` suffixes to Windows targets.
      data: Additional runtime data for the Linux test.
      target_compatible_with: Additional compatibility constraints.
      **kwargs: Remaining attributes forwarded to `rust_test`.
    """
    binaries = dict(_WINE_RUNTIME_BINARIES)
    for index, binary_name in enumerate(sorted(windows_binaries.keys())):
        if binary_name in binaries:
            fail("Windows test binary name collides with Wine runtime: {}".format(binary_name))
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

    rust_test(
        name = name,
        data = data + [
            "@wine_linux_x86_64//:runtime",
        ] + [binary for binary in binaries.values()],
        env = {
            "CARGO_BIN_EXE_{}".format(binary_name): "$(rlocationpath {})".format(binary)
            for binary_name, binary in binaries.items()
        },
        target_compatible_with = target_compatible_with + [
            "@llvm//constraints/libc:gnu.2.28",
            "@platforms//cpu:x86_64",
            "@platforms//os:linux",
        ],
        **kwargs
    )
