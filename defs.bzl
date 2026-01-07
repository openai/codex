load("@crates//:defs.bzl", "all_crate_deps")
load("@crates//:data.bzl", "DEP_DATA")
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

def codex_rust_crate(
        name,
        crate_name,
        crate_features = [],
        crate_srcs = None,
        compile_data = [],
        deps_extra = [],
        proc_macro_deps_extra = [],
        dev_deps_extra = [],
        dev_proc_macro_deps_extra = [],
        integration_deps_extra = [],
        integration_compile_data_extra = [],
        test_data_extra = [],
        visibility = ["//visibility:public"]):

    deps = all_crate_deps(normal = True) + deps_extra
    dev_deps = all_crate_deps(normal_dev = True) + dev_deps_extra
    proc_macro_deps = all_crate_deps(proc_macro = True) + proc_macro_deps_extra
    proc_macro_dev_deps = all_crate_deps(proc_macro_dev = True) + dev_proc_macro_deps_extra

    rust_library(
        name = name,
        crate_name = crate_name,
        crate_features = crate_features,
        deps = deps,
        proc_macro_deps = proc_macro_deps,
        compile_data = compile_data,
        srcs = crate_srcs if crate_srcs else native.glob(["src/**/*.rs"]),
        visibility = visibility,
    )

    rust_test(
        name = name + "-unit-tests",
        crate = name,
        deps = deps + dev_deps,
        proc_macro_deps = proc_macro_deps + proc_macro_dev_deps,
    )

    binaries = DEP_DATA.get(native.package_name())["binaries"]
    
    for binary, main in binaries.items():
        rust_binary(
            name = binary,
            crate_name = binary,
            deps = [name] + deps,
            proc_macro_deps = proc_macro_deps,
            srcs = native.glob(["src/**/*.rs"]),
        )

    for test in native.glob(["tests/*.rs"], allow_empty = True):
        test_name = name + "-" + test.removeprefix("tests/").removesuffix(".rs").replace("/", "-")
        if not test_name.endswith("-test"):
            test_name += "-test"

        rust_test(
            name = test_name,
            crate_root = test,
            srcs = [test],
            data = native.glob(["tests/**"], allow_empty = True) + binaries.keys() + test_data_extra,
            compile_data = native.glob(["tests/**"], allow_empty = True) + integration_compile_data_extra,
            deps = [name] + deps + dev_deps + integration_deps_extra,
            proc_macro_deps = proc_macro_deps + proc_macro_dev_deps,
            env = {
                "CARGO_BIN_EXE_" + binary: "$(rootpath :%s)" % binary
                for binary in binaries
            },
        )
