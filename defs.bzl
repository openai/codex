load("@crates//:defs.bzl", "all_crate_deps")
load("@rules_platform//platform_data:defs.bzl", "platform_data")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_test")

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

    for platform in platforms:
        platform_data(
            name = name + "_" + platform,
            platform = "@toolchains_llvm_bootstrapped//platforms:" + platform,
            target = name,
        )

def codex_rust_crate(name, crate_name):
    deps = all_crate_deps(normal = True)
    dev_deps = all_crate_deps(normal_dev = True)
    proc_macro_deps = all_crate_deps(proc_macro = True)
    proc_macro_dev_deps = all_crate_deps(proc_macro_dev = True)

    rust_library(
        name = name,
        crate_name = crate_name,
        deps = deps,
        proc_macro_deps = proc_macro_deps,
        srcs = native.glob(["src/**/*.rs"]),
        visibility = ["//visibility:public"],
    )

    rust_test(
        name = name + "-tests",
        crate = name,
        deps = deps + dev_deps,
        proc_macro_deps = proc_macro_deps + proc_macro_dev_deps,
    )

    for test in native.glob(["tests/**/*.rs"]):
        rust_test(
            name = name + "-" + test.removeprefix("tests/").removesuffix(".rs").replace("/", "-"),
            crate_root = test,
            srcs = [test],
            data = native.glob(["tests/**"]),
            deps = [name] + deps + dev_deps,
            proc_macro_deps = proc_macro_deps + proc_macro_dev_deps,
        )
