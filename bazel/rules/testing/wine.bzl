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

    Values in `windows_binaries` must be executable targets. Each target is
    transitioned to the GNU/LLVM Windows platform, where every Rust target in
    its dependency graph receives the repository's Windows linker flags. The
    test itself stays on x86-64 Linux.

    The generated test has this environment-variable contract:

    * Each `windows_binaries` entry contributes a
      `CARGO_BIN_EXE_<binary_name>` variable for its transitioned executable.
    * `CARGO_BIN_EXE_wine` and `CARGO_BIN_EXE_wineserver` identify the matching
      Wine host executables.
    * `CARGO_BIN_EXE_wine-runtime-marker` identifies a file whose parent is the
      Wine DLL directory to use as `WINEDLLPATH`.

    These values are Bazel runfile locations, not necessarily filesystem paths.
    Rust tests should resolve the Windows binary with
    `codex_utils_cargo_bin::cargo_bin`. The reusable
    `//bazel/rules/testing/wine:wine_test_support` library resolves the three
    fixed Wine runtime names and starts each process in an isolated prefix.

    Args:
      name: Name of the generated Linux `rust_test`.
      windows_binaries: Map from `CARGO_BIN_EXE_*` suffixes to executable
        targets that should be built for Windows.
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
