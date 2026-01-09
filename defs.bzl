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
        integration_deps_extra = [],
        integration_compile_data_extra = [],
        test_data_extra = [],
        test_tags = [],
        extra_binaries = []):
    """Defines a Rust crate with library, binaries, and tests wired for Bazel + Cargo parity.

    The macro mirrors Cargo conventions: it builds a library when `src/` exists,
    wires build scripts, exports `CARGO_BIN_EXE_*` for integration tests, and
    creates unit + integration test targets. Dependency buckets map to the
    Cargo.lock resolution in `@crates`.

    Args:
        name: Bazel target name for the library, should be the directory name.
            Example: `app-server`.
        crate_name: Cargo crate name from Cargo.toml
            Example: `codex_app_server`.
        crate_features: Cargo features to enable for this crate.
            Crates are only compiled in a single configuration across the workspace, i.e.
            with all features in this list enabled. So use sparingly, and prefer to refactor
            optional functionality to a separate crate.
        crate_srcs: Optional explicit srcs; defaults to `src/**/*.rs`.
        crate_edition: Rust edition override, if not default.
            You probably don't want this, it's only here for a single caller.
        build_script_data: Data files exposed to the build script at runtime.
        compile_data: Non-Rust compile-time data for the library target.
        deps_extra: Extra normal deps beyond @crates resolution.
            Typically only needed when features add additional deps.
        integration_deps_extra: Extra deps for integration tests only.
        integration_compile_data_extra: Extra compile_data for integration tests.
        test_data_extra: Extra runtime data for tests.
        test_tags: Tags applied to unit + integration test targets.
            Typically used to disable the sandbox, but see https://bazel.build/reference/be/common-definitions#common.tags
        extra_binaries: Additional binary labels to surface as test data and
            `CARGO_BIN_EXE_*` environment variables. These are only needed for binaries from a different crate.
    """
    deps = all_crate_deps(normal = True) + deps_extra
    dev_deps = all_crate_deps(normal_dev = True)
    proc_macro_deps = all_crate_deps(proc_macro = True)
    proc_macro_dev_deps = all_crate_deps(proc_macro_dev = True)

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
