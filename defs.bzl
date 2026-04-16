load("@crates//:data.bzl", "DEP_DATA")
load("@crates//:defs.bzl", "all_crate_deps")
load("@rules_platform//platform_data:defs.bzl", "platform_data")
load("@rules_rust//cargo/private:cargo_build_script_wrapper.bzl", "cargo_build_script")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_proc_macro", "rust_test")

PLATFORMS = [
    "linux_arm64_musl",
    "linux_amd64_musl",
    "macos_amd64",
    "macos_arm64",
    "windows_amd64",
    "windows_arm64",
]

# Match Cargo's Windows linker behavior so Bazel-built binaries and tests use
# the same stack reserve on both Windows ABIs and resolve UCRT imports on MSVC.
WINDOWS_RUSTC_LINK_FLAGS = select({
    "@rules_rs//rs/experimental/platforms/constraints:windows_gnullvm": [
        "-C",
        "link-arg=-Wl,--stack,8388608",
    ],
    "@rules_rs//rs/experimental/platforms/constraints:windows_msvc": [
        "-C",
        "link-arg=/STACK:8388608",
        "-C",
        "link-arg=/NODEFAULTLIB:libucrt.lib",
        "-C",
        "link-arg=ucrt.lib",
    ],
    "//conditions:default": [],
})

# libwebrtc uses Objective-C categories from native archives. Any Bazel-linked
# macOS binary/test that can pull it in must keep category symbols alive.
MACOS_WEBRTC_RUSTC_LINK_FLAGS = select({
    "@platforms//os:macos": [
        "-C",
        "link-arg=-ObjC",
        "-C",
        "link-arg=-lc++",
    ],
    "//conditions:default": [],
})

def multiplatform_binaries(name, platforms = PLATFORMS):
    for platform in platforms:
        platform_data(
            name = name + "_" + platform,
            platform = "@llvm//platforms:" + platform,
            target = name,
            tags = ["manual"],
        )

    native.filegroup(
        name = "release_binaries",
        srcs = [name + "_" + platform for platform in platforms],
        tags = ["manual"],
    )

def _workspace_root_test_impl(ctx):
    is_windows = ctx.target_platform_has_constraint(ctx.attr._windows_constraint[platform_common.ConstraintValueInfo])
    launcher = ctx.actions.declare_file(ctx.label.name + ".bat" if is_windows else ctx.label.name)
    test_bin = ctx.executable.test_bin
    workspace_root_marker = ctx.file.workspace_root_marker
    launcher_template = ctx.file._windows_launcher_template if is_windows else ctx.file._bash_launcher_template
    ctx.actions.expand_template(
        template = launcher_template,
        output = launcher,
        is_executable = True,
        substitutions = {
            "__TEST_BIN__": test_bin.short_path,
            "__WORKSPACE_ROOT_MARKER__": workspace_root_marker.short_path,
        },
    )

    runfiles = ctx.runfiles(files = [test_bin, workspace_root_marker]).merge(ctx.attr.test_bin[DefaultInfo].default_runfiles)
    for data_dep in ctx.attr.data:
        runfiles = runfiles.merge(ctx.runfiles(files = data_dep[DefaultInfo].files.to_list()))
        runfiles = runfiles.merge(data_dep[DefaultInfo].default_runfiles)

    return [
        DefaultInfo(
            executable = launcher,
            files = depset([launcher]),
            runfiles = runfiles,
        ),
        RunEnvironmentInfo(
            environment = ctx.attr.env,
        ),
    ]

workspace_root_test = rule(
    implementation = _workspace_root_test_impl,
    test = True,
    attrs = {
        "data": attr.label_list(
            allow_files = True,
        ),
        "env": attr.string_dict(),
        "test_bin": attr.label(
            cfg = "target",
            executable = True,
            mandatory = True,
        ),
        "workspace_root_marker": attr.label(
            allow_single_file = True,
            mandatory = True,
        ),
        "_windows_constraint": attr.label(
            default = "@platforms//os:windows",
            providers = [platform_common.ConstraintValueInfo],
        ),
        "_bash_launcher_template": attr.label(
            allow_single_file = True,
            default = "//:workspace_root_test_launcher.sh.tpl",
        ),
        "_windows_launcher_template": attr.label(
            allow_single_file = True,
            default = "//:workspace_root_test_launcher.bat.tpl",
        ),
    },
)

def codex_rust_crate(
        name,
        crate_name,
        crate_features = [],
        crate_srcs = None,
        crate_edition = None,
        proc_macro = False,
        build_script_enabled = True,
        build_script_data = [],
        compile_data = [],
        lib_data_extra = [],
        rustc_flags_extra = [],
        rustc_env = {},
        deps_extra = [],
        integration_compile_data_extra = [],
        integration_test_args = [],
        integration_test_timeout = None,
        test_data_extra = [],
        test_shard_counts = {},
        test_tags = [],
        unit_test_timeout = None,
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
        proc_macro: Whether this crate builds a proc-macro library.
        build_script_data: Data files exposed to the build script at runtime.
        compile_data: Non-Rust compile-time data for the library target.
        lib_data_extra: Extra runtime data for the library target.
        rustc_env: Extra rustc_env entries to merge with defaults.
        deps_extra: Extra normal deps beyond @crates resolution.
            Typically only needed when features add additional deps.
        integration_compile_data_extra: Extra compile_data for integration tests.
        integration_test_args: Optional args for integration test binaries.
        integration_test_timeout: Optional Bazel timeout for integration test
            targets generated from `tests/*.rs`.
        test_data_extra: Extra runtime data for tests.
        test_shard_counts: Mapping from generated test target name to Bazel
            shard count. Matching tests use native Bazel sharding on the
            original test label, while rules_rust assigns each Rust test case
            to a stable bucket by hashing the test name.
        test_tags: Tags applied to unit + integration test targets.
            Typically used to disable the sandbox, but see https://bazel.build/reference/be/common-definitions#common.tags
        unit_test_timeout: Optional Bazel timeout for the unit-test target
            generated from `src/**/*.rs`.
        extra_binaries: Additional binary labels to surface as test data and
            `CARGO_BIN_EXE_*` environment variables. These are only needed for binaries from a different crate.
    """
    test_env = {
        # The launcher resolves an absolute workspace root at runtime so
        # manifest-only platforms like macOS still point Insta at the real
        # `codex-rs` checkout.
        "INSTA_WORKSPACE_ROOT": ".",
        "INSTA_SNAPSHOT_PATH": "src",
    }

    native.filegroup(
        name = "package-files",
        srcs = native.glob(
            ["**"],
            exclude = [
                "**/BUILD.bazel",
                "BUILD.bazel",
                "target/**",
            ],
            allow_empty = True,
        ),
        visibility = ["//visibility:public"],
    )

    rustc_env = {
        "BAZEL_PACKAGE": native.package_name(),
    } | rustc_env

    manifest_relpath = native.package_name()
    if manifest_relpath.startswith("codex-rs/"):
        manifest_relpath = manifest_relpath[len("codex-rs/"):]
    manifest_path = manifest_relpath + "/Cargo.toml"

    binaries = DEP_DATA.get(native.package_name())["binaries"]

    lib_srcs = crate_srcs or native.glob(["src/**/*.rs"], exclude = binaries.values(), allow_empty = True)

    maybe_deps = []

    if build_script_enabled and native.glob(["build.rs"], allow_empty = True):
        cargo_build_script(
            name = name + "-build-script",
            srcs = ["build.rs"],
            deps = all_crate_deps(build = True),
            data = build_script_data,
            # Some build script deps sniff version-related env vars...
            version = "0.0.0",
        )

        maybe_deps += [name + "-build-script"]

    if lib_srcs:
        lib_rule = rust_proc_macro if proc_macro else rust_library
        lib_rule(
            name = name,
            crate_name = crate_name,
            crate_features = crate_features,
            deps = all_crate_deps() + maybe_deps + deps_extra,
            compile_data = compile_data,
            data = lib_data_extra,
            srcs = lib_srcs,
            edition = crate_edition,
            rustc_flags = rustc_flags_extra,
            rustc_env = rustc_env,
            visibility = ["//visibility:public"],
        )

        unit_test_name = name + "-unit-tests"
        unit_test_binary = name + "-unit-tests-bin"
        unit_test_shard_count = _test_shard_count(test_shard_counts, unit_test_name)
        unit_test_binary_kwargs = {}
        if unit_test_shard_count:
            unit_test_binary_kwargs["experimental_enable_sharding"] = True

        rust_test(
            name = unit_test_binary,
            crate = name,
            deps = all_crate_deps(normal = True, normal_dev = True) + maybe_deps + deps_extra,
            # Unit tests also compile to standalone Windows executables, so
            # keep their stack reserve aligned with binaries and integration
            # tests under gnullvm.
            # Bazel has emitted both `codex-rs/<crate>/...` and
            # `../codex-rs/<crate>/...` paths for `file!()`. Strip either
            # prefix so the workspace-root launcher sees Cargo-like metadata
            # such as `tui/src/...`.
            rustc_flags = rustc_flags_extra + WINDOWS_RUSTC_LINK_FLAGS + [
                "--remap-path-prefix=../codex-rs=",
                "--remap-path-prefix=codex-rs=",
            ],
            rustc_env = rustc_env,
            data = test_data_extra,
            tags = test_tags + ["manual"],
            **unit_test_binary_kwargs
        )

        unit_test_kwargs = {}
        if unit_test_timeout:
            unit_test_kwargs["timeout"] = unit_test_timeout
        if unit_test_shard_count:
            unit_test_kwargs["shard_count"] = unit_test_shard_count

        workspace_root_test(
            name = unit_test_name,
            env = test_env,
            test_bin = ":" + unit_test_binary,
            workspace_root_marker = "//codex-rs/utils/cargo-bin:repo_root.marker",
            tags = test_tags,
            **unit_test_kwargs
        )

        maybe_deps += [name]

    sanitized_binaries = []
    cargo_env = {}
    for binary, main in binaries.items():
        #binary = binary.replace("-", "_")
        sanitized_binaries.append(binary)
        cargo_env["CARGO_BIN_EXE_" + binary] = "$(rlocationpath :%s)" % binary

        rust_binary(
            name = binary,
            crate_name = binary.replace("-", "_"),
            crate_root = main,
            deps = all_crate_deps() + maybe_deps + deps_extra,
            edition = crate_edition,
            rustc_flags = rustc_flags_extra + WINDOWS_RUSTC_LINK_FLAGS,
            srcs = native.glob(["src/**/*.rs"]),
            visibility = ["//visibility:public"],
        )

    for binary_label in extra_binaries:
        sanitized_binaries.append(binary_label)
        binary = Label(binary_label).name
        cargo_env["CARGO_BIN_EXE_" + binary] = "$(rlocationpath %s)" % binary_label

    integration_test_kwargs = {}
    if integration_test_args:
        integration_test_kwargs["args"] = integration_test_args
    if integration_test_timeout:
        integration_test_kwargs["timeout"] = integration_test_timeout

    for test in native.glob(["tests/*.rs"], allow_empty = True):
        test_file_stem = test.removeprefix("tests/").removesuffix(".rs")
        test_crate_name = test_file_stem.replace("-", "_")
        test_name = name + "-" + test_file_stem.replace("/", "-")
        if not test_name.endswith("-test"):
            test_name += "-test"

        test_kwargs = {}
        test_kwargs.update(integration_test_kwargs)
        test_shard_count = _test_shard_count(test_shard_counts, test_name)
        if test_shard_count:
            test_kwargs["experimental_enable_sharding"] = True
            test_kwargs["shard_count"] = test_shard_count

        rust_test(
            name = test_name,
            crate_name = test_crate_name,
            crate_root = test,
            srcs = [test],
            data = native.glob(["tests/**"], allow_empty = True) + sanitized_binaries + test_data_extra,
            compile_data = native.glob(["tests/**"], allow_empty = True) + integration_compile_data_extra,
            deps = all_crate_deps(normal = True, normal_dev = True) + maybe_deps + deps_extra,
            # Bazel has emitted both `codex-rs/<crate>/...` and
            # `../codex-rs/<crate>/...` paths for `file!()`. Strip either
            # prefix so Insta records Cargo-like metadata such as `core/tests/...`.
            rustc_flags = rustc_flags_extra + WINDOWS_RUSTC_LINK_FLAGS + [
                "--remap-path-prefix=../codex-rs=",
                "--remap-path-prefix=codex-rs=",
            ],
            rustc_env = rustc_env,
            # Important: do not merge `test_env` here. Its unit-test-only
            # `INSTA_WORKSPACE_ROOT="codex-rs"` is tuned for unit tests that
            # execute from the repo root and can misplace integration snapshots.
            env = cargo_env,
            tags = test_tags,
            **test_kwargs
        )

def _test_shard_count(test_shard_counts, test_name):
    shard_count = test_shard_counts.get(test_name)
    if shard_count == None:
        return None

    if shard_count < 1:
        fail("test_shard_counts[{}] must be a positive integer".format(test_name))

    return shard_count
