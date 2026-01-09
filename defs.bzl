load("@crates//:data.bzl", "DEP_DATA")
load("@crates//:defs.bzl", "all_crate_deps")
load("@rules_platform//platform_data:defs.bzl", "platform_data")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_test")
load("@rules_rust//cargo/private:cargo_build_script_wrapper.bzl", "cargo_build_script")

PLATFORMS = [
    "linux_arm64_musl",
    "linux_amd64_musl",
    "macos_amd64",
    "macos_arm64",
    "windows_amd64",
    "windows_arm64",
]

def multiplatform_binaries(name, platforms = PLATFORMS):
    for platform in platforms:
        platform_data(
            name = name + "_" + platform,
            platform = "@toolchains_llvm_bootstrapped//platforms:" + platform,
            target = name,
            tags = ["manual"],
        )

    native.filegroup(
        name = "release_binaries",
        srcs = [name + "_" + platform for platform in platforms],
        tags = ["manual"],
    )

def codex_rust_crate(
        name,
        crate_name,
        crate_features = [],
        crate_srcs = None,
        crate_edition = None,
        build_script_data = [],
        compile_data = [],
        deps_extra = [],
        proc_macro_deps_extra = [],
        dev_deps_extra = [],
        dev_proc_macro_deps_extra = [],
        integration_deps_extra = [],
        integration_compile_data_extra = [],
        test_data_extra = [],
        test_tags = [],
        extra_binaries = [],
        visibility = ["//visibility:public"]):
    deps = all_crate_deps(normal = True) + deps_extra
    dev_deps = all_crate_deps(normal_dev = True) + dev_deps_extra
    proc_macro_deps = all_crate_deps(proc_macro = True) + proc_macro_deps_extra
    proc_macro_dev_deps = all_crate_deps(proc_macro_dev = True) + dev_proc_macro_deps_extra

    test_env = {
        "INSTA_WORKSPACE_ROOT": ".",
        "INSTA_SNAPSHOT_PATH": "src",
    }

    rustc_env = {
        "BAZEL_PACKAGE": native.package_name(),
    }

    binaries = DEP_DATA.get(native.package_name())["binaries"]

    # TODO(zbarsky): cargo_build_script support?

    lib_srcs = crate_srcs or native.glob(["src/**/*.rs"], exclude = binaries.values(), allow_empty = True)

    if native.glob(["build.rs"], allow_empty = True):
        cargo_build_script(
            name = name + "-build-script",
            srcs = ["build.rs"],
            deps = all_crate_deps(build = True),
            proc_macro_deps = all_crate_deps(build_proc_macro = True),
            data = build_script_data,
            # Some build script deps sniff version-related env vars...
            version = "0.0.0",
        )

        deps = deps + [name + "-build-script"]

    if lib_srcs:
        rust_library(
            name = name,
            crate_name = crate_name,
            crate_features = crate_features,
            deps = deps,
            proc_macro_deps = proc_macro_deps,
            compile_data = compile_data,
            srcs = lib_srcs,
            edition = crate_edition,
            rustc_env = rustc_env,
            visibility = ["//visibility:public"],
        )

        rust_test(
            name = name + "-unit-tests",
            crate = name,
            env = test_env,
            deps = deps + dev_deps,
            proc_macro_deps = proc_macro_deps + proc_macro_dev_deps,
            rustc_env = rustc_env,
            data = test_data_extra,
            tags = test_tags,
        )

        maybe_lib = [name]
    else:
        maybe_lib = []

    sanitized_binaries = []
    cargo_env = {}
    for binary, main in binaries.items():
        #binary = binary.replace("-", "_")
        sanitized_binaries.append(binary)
        cargo_env["CARGO_BIN_EXE_" + binary] = "$(rootpath :%s)" % binary

        rust_binary(
            name = binary,
            crate_name = binary.replace("-", "_"),
            crate_root = main,
            deps = maybe_lib + deps,
            proc_macro_deps = proc_macro_deps,
            edition = crate_edition,
            srcs = native.glob(["src/**/*.rs"]),
            visibility = ["//visibility:public"],
        )

    for binary_label in extra_binaries:
        sanitized_binaries.append(binary_label)
        binary = Label(binary_label).name
        cargo_env["CARGO_BIN_EXE_" + binary] = "$(rootpath %s)" % binary_label

    for test in native.glob(["tests/*.rs"], allow_empty = True):
        test_name = name + "-" + test.removeprefix("tests/").removesuffix(".rs").replace("/", "-")
        if not test_name.endswith("-test"):
            test_name += "-test"

        rust_test(
            name = test_name,
            crate_root = test,
            srcs = [test],
            data = native.glob(["tests/**"], allow_empty = True) + sanitized_binaries + test_data_extra,
            compile_data = native.glob(["tests/**"], allow_empty = True) + integration_compile_data_extra,
            deps = maybe_lib + deps + dev_deps + integration_deps_extra,
            proc_macro_deps = proc_macro_deps + proc_macro_dev_deps,
            rustc_env = rustc_env,
            env = test_env | cargo_env,
            tags = test_tags,
        )
