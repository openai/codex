"""Runfiles shared by tests that execute Windows binaries through Wine."""

_WINE_RUNTIME_BINARIES = {
    "pwsh": "@powershell_windows_x86_64//:pwsh",
    "pwsh-runtime-marker": "@powershell_windows_x86_64//:runtime_marker",
    "wine": "@wine_linux_x86_64//:wine",
    "wine-runtime-marker": "@wine_linux_x86_64//:runtime_marker",
    "wine-test-exec": "//bazel/rules/testing/wine:wine-test-exec",
    "wineserver": "@wine_linux_x86_64//:wineserver",
}

_WINE_RUNTIME_DATA = [
    "@powershell_windows_x86_64//:runtime",
    "@wine_linux_x86_64//:runtime",
]

WINE_TEST_TARGET_COMPATIBLE_WITH = [
    "@llvm//constraints/libc:gnu.2.28",
    "@platforms//cpu:x86_64",
    "@platforms//os:linux",
]

def wine_test_runtime(test_binaries = {}):
    """Returns data and environment mappings for a Wine-backed test."""
    binaries = dict(_WINE_RUNTIME_BINARIES)
    for binary_name in sorted(test_binaries.keys()):
        if binary_name in binaries:
            fail("test binary name collides with Wine runtime: {}".format(binary_name))
        binaries[binary_name] = test_binaries[binary_name]

    return struct(
        data = _WINE_RUNTIME_DATA + [binary for binary in binaries.values()],
        env = {
            "CARGO_BIN_EXE_{}".format(binary_name): "$(rlocationpath {})".format(binary)
            for binary_name, binary in binaries.items()
        },
        runfile_env = {
            binary_label: "CARGO_BIN_EXE_" + binary_name
            for binary_name, binary_label in binaries.items()
        },
    )
