load("@rules_platform//platform_data:defs.bzl", "platform_data")
load("@rules_rust//rust:defs.bzl", "rust_binary")

PLATFORMS = [
    "linux_arm64_musl",
    "linux_amd64_musl",
    "macos_amd64",
    "macos_arm64",
    "windows_amd64",
    "windows_arm64",
]

def rust_release_binary(name, platforms = PLATFORMS, **kwargs):
    rust_binary(
        name = name,
        **kwargs
    )

    for platform in PLATFORMS:
        platform_data(
            name = name + "_" + platform,
            platform = "@toolchains_llvm_bootstrapped//platforms:" + platform,
            target = name,
        )
