load("@crates//:data.bzl", "DEP_DATA")
load("@crates//:defs.bzl", "all_crate_deps")
load("@rules_rust//cargo/private:cargo_build_script_wrapper.bzl", "cargo_build_script")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_proc_macro", "rust_test")
load("//bazel/rules/testing:foreign_platform_binary.bzl", "foreign_platform_binary")
load("//bazel/rules/testing/wine:wine_runtime.bzl", "WINE_TEST_TARGET_COMPATIBLE_WITH", "wine_test_runtime")

# Match Cargo's Windows linker behavior so Bazel-built binaries and tests use
# the same stack reserve on both Windows ABIs and resolve UCRT imports on MSVC.
WINDOWS_GNULLVM_RUSTC_LINK_FLAGS = [
    "-C",
    "link-arg=-Wl,--stack,8388608",  # 8 MiB
]

WINDOWS_RUSTC_LINK_FLAGS = select({
    "@rules_rs//rs/experimental/platforms/constraints:windows_gnullvm": WINDOWS_GNULLVM_RUSTC_LINK_FLAGS,
    "@rules_rs//rs/experimental/platforms/constraints:windows_msvc": [
        "-C",
        "link-arg=/STACK:8388608",  # 8 MiB
        "-C",
        "link-arg=/NODEFAULTLIB:libucrt.lib",
        "-C",
        "link-arg=ucrt.lib",
    ],
    "//conditions:default": [],
})

WINDOWS_GNULLVM_INCOMPATIBLE = select({
    "@rules_rs//rs/experimental/platforms/constraints:windows_gnullvm": ["@platforms//:incompatible"],
    "//conditions:default": [],
})

WINDOWS_GNULLVM_ONLY = select({
    "@rules_rs//rs/experimental/platforms/constraints:windows_gnullvm": [],
    "//conditions:default": ["@platforms//:incompatible"],
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

def _workspace_root_test_impl(ctx):
    is_windows = ctx.target_platform_has_constraint(ctx.attr._windows_constraint[platform_common.ConstraintValueInfo])
    launcher = ctx.actions.declare_file(ctx.label.name + ".bat" if is_windows else ctx.label.name)
    test_bin = ctx.executable.test_bin
    workspace_root_marker = ctx.file.workspace_root_marker
    launcher_template = ctx.file._windows_launcher_template if is_windows else ctx.file._bash_launcher_template
    runfile_env_exports = _windows_runfile_env_exports(ctx) if is_windows else _bash_runfile_env_exports(ctx)
    workspace_root_setup = _windows_workspace_root_setup(ctx) if is_windows else _bash_workspace_root_setup(ctx)
    ctx.actions.expand_template(
        template = launcher_template,
        output = launcher,
        is_executable = True,
        substitutions = {
            "__RUNFILE_ENV_EXPORTS__": runfile_env_exports,
            "__TEST_BIN__": test_bin.short_path,
            "__WORKSPACE_ROOT_SETUP__": workspace_root_setup,
            "__WORKSPACE_ROOT_MARKER__": workspace_root_marker.short_path,
        },
    )

    runfiles = ctx.runfiles(files = [test_bin, workspace_root_marker]).merge(ctx.attr.test_bin[DefaultInfo].default_runfiles)
    for data_dep in ctx.attr.data:
        runfiles = runfiles.merge(ctx.runfiles(files = data_dep[DefaultInfo].files.to_list()))
        runfiles = runfiles.merge(data_dep[DefaultInfo].default_runfiles)
    for runfile_dep in ctx.attr.runfile_env:
        runfile = _runfile_env_file(runfile_dep)
        runfiles = runfiles.merge(ctx.runfiles(files = [runfile]))
        runfiles = runfiles.merge(runfile_dep[DefaultInfo].default_runfiles)

    location_targets = {}
    for target in (
        ctx.attr.data +
        [ctx.attr.test_bin, ctx.attr.workspace_root_marker] +
        ctx.attr.runfile_env.keys()
    ):
        location_targets[target.label] = target
    env = {
        key: ctx.expand_location(value, targets = location_targets.values())
        for key, value in ctx.attr.env.items()
    }

    return [
        DefaultInfo(
            executable = launcher,
            files = depset([launcher]),
            runfiles = runfiles,
        ),
        RunEnvironmentInfo(
            environment = env,
        ),
    ]

def _bash_runfile_env_exports(ctx):
    lines = []
    for runfile_dep, env_var in ctx.attr.runfile_env.items():
        runfile = _runfile_env_file(runfile_dep)
        lines.append('RUNFILE_ENV_ARGS+=("{}=$(resolve_runfile "{}")")'.format(env_var, _runfile_logical_path(runfile)))
    return "\n".join(lines)

def _windows_runfile_env_exports(ctx):
    lines = []
    for runfile_dep, env_var in ctx.attr.runfile_env.items():
        runfile = _runfile_env_file(runfile_dep)
        lines.append('call :resolve_runfile {} "{}"'.format(env_var, _runfile_logical_path(runfile)))
        lines.append("if errorlevel 1 exit /b 1")
    return "\n".join(lines)

def _runfile_env_file(target):
    executable = target[DefaultInfo].files_to_run.executable
    if executable != None:
        return executable

    files = target[DefaultInfo].files.to_list()
    if len(files) != 1:
        fail("{} must provide an executable or exactly one file for runfile_env".format(target.label))
    return files[0]

def _runfile_logical_path(file):
    return file.short_path.removeprefix("../")

def _bash_workspace_root_setup(ctx):
    if not ctx.attr.chdir_workspace_root:
        return ""
    return 'export INSTA_WORKSPACE_ROOT="${workspace_root}"\ncd "${workspace_root}"'

def _windows_workspace_root_setup(ctx):
    if not ctx.attr.chdir_workspace_root:
        return ""
    return """set "INSTA_WORKSPACE_ROOT=%workspace_root%"
cd /d "%workspace_root%" || exit /b 1"""

workspace_root_test = rule(
    implementation = _workspace_root_test_impl,
    test = True,
    toolchains = ["@bazel_tools//tools/test:default_test_toolchain_type"],
    attrs = {
        "chdir_workspace_root": attr.bool(
            default = True,
        ),
        "data": attr.label_list(
            allow_files = True,
        ),
        "env": attr.string_dict(),
        "runfile_env": attr.label_keyed_string_dict(
            cfg = "target",
        ),
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
        test_sizes = {},
        test_tags = [],
        unit_test_timeout = None,
        extra_binaries = [],
        extra_binaries_non_windows = [],
        run_tests_with_wine_exec = False):
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
            shard count. Matching tests use native Bazel sharding on the outer
            workspace-root launcher, not rules_rust's inner sharding wrapper.
            The launcher resolves the real Rust test binary through runfiles
            and then assigns each libtest case to a stable bucket by hashing
            the test name. Matching tests are also marked flaky, which gives
            them Bazel's default three attempts. Windows-cross and Wine
            variants inherit the native target's count unless callers
            override the generated variant name explicitly.
        test_sizes: Mapping from generated test target name to Bazel test size.
            All generated tests default to small. Callers can override slower
            targets individually, including Windows-cross and Wine tests.
        test_tags: Tags applied to unit + integration test targets.
            Typically used to disable the sandbox, but see https://bazel.build/reference/be/common-definitions#common.tags
        unit_test_timeout: Optional Bazel timeout for the unit-test target
            generated from `src/**/*.rs`.
        extra_binaries: Additional binary labels to surface as test data and
            `CARGO_BIN_EXE_*` environment variables. These are only needed for binaries from a different crate.
        extra_binaries_non_windows: Like `extra_binaries`, but omitted from
            Windows test data and environment variables. Tests using these
            binaries must be excluded when targeting Windows.
        run_tests_with_wine_exec: Boolean, defaults to False. Whether to emit a
            Wine-exec variant for each integration test. Variants inherit the
            native test's timeout, tags, and shard count.
    """
    test_env = {
        # The launcher resolves an absolute workspace root at runtime so
        # manifest-only platforms like macOS still point Insta at the real
        # `codex-rs` checkout.
        "INSTA_WORKSPACE_ROOT": ".",
        "INSTA_SNAPSHOT_PATH": "src",
    }
    generated_test_names = []

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
        windows_cross_unit_test_name = unit_test_name + "-windows-cross"
        generated_test_names.extend([unit_test_name, windows_cross_unit_test_name])
        unit_test_shard_count = _test_shard_count(test_shard_counts, unit_test_name)
        windows_cross_unit_test_shard_count = _test_shard_count(test_shard_counts, windows_cross_unit_test_name)
        if windows_cross_unit_test_shard_count == None:
            windows_cross_unit_test_shard_count = unit_test_shard_count

        # Shard at the workspace_root_test layer. rules_rust's sharding wrapper
        # expects to run from its own runfiles cwd, while workspace_root_test
        # deliberately changes cwd so Insta sees Cargo-like snapshot paths.
        rust_test(
            name = unit_test_binary,
            crate = name,
            crate_features = crate_features,
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
        )

        # Keep a distinct Windows-cross label so callers can tune its
        # nonconfigurable test attributes without changing every platform.
        for unit_test_target_name, target_compatible_with, shard_count in [
            (unit_test_name, WINDOWS_GNULLVM_INCOMPATIBLE, unit_test_shard_count),
            (windows_cross_unit_test_name, WINDOWS_GNULLVM_ONLY, windows_cross_unit_test_shard_count),
        ]:
            unit_test_kwargs = {
                "size": _test_size(test_sizes, unit_test_target_name),
            }
            if unit_test_timeout:
                unit_test_kwargs["timeout"] = unit_test_timeout
            if shard_count:
                unit_test_kwargs["shard_count"] = shard_count
                unit_test_kwargs["flaky"] = True

            workspace_root_test(
                name = unit_test_target_name,
                env = test_env,
                test_bin = ":" + unit_test_binary,
                workspace_root_marker = "//codex-rs/utils/cargo-bin:repo_root.marker",
                target_compatible_with = target_compatible_with,
                tags = test_tags,
                **unit_test_kwargs
            )

        maybe_deps += [name]

    sanitized_binaries = []
    cargo_env = {}
    cargo_env_runfiles = {}
    for binary, main in binaries.items():
        #binary = binary.replace("-", "_")
        sanitized_binaries.append(binary)
        cargo_env_runfiles[":" + binary] = "CARGO_BIN_EXE_" + binary
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
        cargo_env_runfiles[binary_label] = "CARGO_BIN_EXE_" + binary
        cargo_env["CARGO_BIN_EXE_" + binary] = "$(rlocationpath %s)" % binary_label

    integration_test_binaries = sanitized_binaries
    integration_test_cargo_env = cargo_env
    integration_test_cargo_env_runfiles = cargo_env_runfiles
    non_windows_sanitized_binaries = []
    non_windows_cargo_env = {}
    non_windows_cargo_env_runfiles = {}
    if extra_binaries_non_windows:
        for binary_label in extra_binaries_non_windows:
            non_windows_sanitized_binaries.append(binary_label)
            binary = Label(binary_label).name
            non_windows_cargo_env_runfiles[binary_label] = "CARGO_BIN_EXE_" + binary
            non_windows_cargo_env["CARGO_BIN_EXE_" + binary] = "$(rlocationpath %s)" % binary_label

        integration_test_binaries = sanitized_binaries + select({
            "@platforms//os:windows": [],
            "//conditions:default": non_windows_sanitized_binaries,
        })
        integration_test_cargo_env = select({
            "@platforms//os:windows": cargo_env,
            "//conditions:default": cargo_env | non_windows_cargo_env,
        })
        integration_test_cargo_env_runfiles = select({
            "@platforms//os:windows": cargo_env_runfiles,
            "//conditions:default": cargo_env_runfiles | non_windows_cargo_env_runfiles,
        })

    wine_host_binaries = {
        env_var.removeprefix("CARGO_BIN_EXE_"): binary_label
        for binary_label, env_var in (cargo_env_runfiles | non_windows_cargo_env_runfiles).items()
    }

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
        windows_cross_test_name = test_name + "-windows-cross"
        windows_cross_test_binary = test_name + "-windows-cross-bin"
        generated_test_names.extend([test_name, windows_cross_test_name])

        test_kwargs = {
            "size": _test_size(test_sizes, test_name),
        }
        test_kwargs.update(integration_test_kwargs)
        test_shard_count = _test_shard_count(test_shard_counts, test_name)
        windows_cross_test_shard_count = _test_shard_count(test_shard_counts, windows_cross_test_name)
        if windows_cross_test_shard_count == None:
            windows_cross_test_shard_count = test_shard_count
        if test_shard_count:
            # Put Bazel sharding on the label users/CI invoke. Do not set
            # rules_rust's experimental_enable_sharding on the Rust test
            # binary: that creates an intermediate wrapper that expects a
            # symlink runfiles tree, while this repo intentionally runs with
            # --noenable_runfiles and usually has only a runfiles manifest.
            test_kwargs["shard_count"] = test_shard_count
            test_kwargs["flaky"] = True

        integration_test_binary = test_name + "-bin"

        # There are four generated integration-test shapes:
        #
        # 1. Unsharded native tests keep the plain rust_test label for minimal
        #    churn and the usual rules_rust Cargo-like environment.
        # 2. Sharded native tests split into a manual rust_test binary plus an
        #    outer workspace_root_test. The outer test action receives Bazel's
        #    sharding environment, resolves the real binary through the
        #    runfiles manifest, and implements stable libtest sharding itself.
        # 3. Windows cross tests always use the workspace_root_test wrapper so
        #    runfile env vars become Windows-native absolute paths before the
        #    Rust process starts.
        # 4. Wine-exec tests reuse the native Rust test binary behind a shared
        #    Linux runner. The runner starts the cross-built Windows exec server
        #    under pinned Wine, injects its URL into the test environment, and
        #    owns cleanup. The outer workspace_root_test resolves the runner,
        #    test, and server from runfiles, sets a Cargo-like cwd, and applies
        #    the native test's shard count.
        if test_shard_count:
            # This target is intentionally a binary-like helper, not the public
            # test target. The wrapper below owns cwd setup, runfile env
            # materialization, sharding, and flaky retry behavior.
            rust_test(
                name = integration_test_binary,
                crate_name = test_crate_name,
                crate_root = test,
                srcs = [test],
                data = native.glob(["tests/**"], allow_empty = True) + integration_test_binaries + test_data_extra,
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
                target_compatible_with = WINDOWS_GNULLVM_INCOMPATIBLE,
                tags = test_tags + ["manual"],
            )

            workspace_root_test(
                name = test_name,
                env = test_env,
                # CARGO_BIN_EXE_* values are rlocation paths at analysis time.
                # The launcher rewrites them to absolute paths at execution
                # time so tests keep working after chdir_workspace_root and on
                # manifest-only platforms.
                runfile_env = integration_test_cargo_env_runfiles,
                test_bin = ":" + integration_test_binary,
                workspace_root_marker = "//codex-rs/utils/cargo-bin:repo_root.marker",
                target_compatible_with = WINDOWS_GNULLVM_INCOMPATIBLE,
                tags = test_tags,
                **test_kwargs
            )
        else:
            # For unsharded tests, the direct rust_test rule is still fine:
            # there is no rules_rust sharding wrapper to bypass, and env can
            # use rlocation paths directly because the test starts under
            # Bazel's normal test environment.
            rust_test(
                name = test_name,
                crate_name = test_crate_name,
                crate_root = test,
                srcs = [test],
                data = native.glob(["tests/**"], allow_empty = True) + integration_test_binaries + test_data_extra,
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
                env = integration_test_cargo_env,
                target_compatible_with = WINDOWS_GNULLVM_INCOMPATIBLE,
                tags = test_tags,
                **test_kwargs
            )

        if run_tests_with_wine_exec:
            wine_test_name = test_name.removesuffix("-test") + "-wine-exec-test"
            generated_test_names.append(wine_test_name)
            native_test_binary = ":" + (integration_test_binary if test_shard_count else test_name)
            wine_test_binaries = dict(wine_host_binaries)

            wine_exec_server = wine_test_name + "-windows-exec-server"
            foreign_platform_binary(
                name = wine_exec_server,
                binary = "//codex-rs/exec-server/testing:windows-exec-server",
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
            wine_test_binaries["wine-windows-exec-server"] = ":" + wine_exec_server
            wine_runtime = wine_test_runtime(wine_test_binaries)
            wine_runfile_env = dict(wine_runtime.runfile_env)
            wine_runfile_env[native_test_binary] = "CODEX_WINE_EXEC_TEST_BINARY"

            wine_test_kwargs = {}
            wine_test_kwargs.update(integration_test_kwargs)
            wine_test_kwargs["size"] = _test_size(test_sizes, wine_test_name)
            wine_test_shard_count = _test_shard_count(test_shard_counts, wine_test_name)
            if wine_test_shard_count == None:
                wine_test_shard_count = test_shard_count
            if wine_test_shard_count:
                wine_test_kwargs["shard_count"] = wine_test_shard_count
                wine_test_kwargs["flaky"] = True

            # The Wine runner is a binary rather than a rust_test, but it still
            # needs a Cargo-like cwd, Bazel sharding, and absolute runfile paths.
            # `workspace_root_test` establishes all three before Wine starts.
            workspace_root_test(
                name = wine_test_name,
                data = wine_runtime.data,
                env = test_env,
                runfile_env = wine_runfile_env,
                test_bin = "//codex-rs/exec-server/testing:wine-exec-test-runner",
                workspace_root_marker = "//codex-rs/utils/cargo-bin:repo_root.marker",
                target_compatible_with = WINE_TEST_TARGET_COMPATIBLE_WITH,
                tags = test_tags + ["manual"],
                **wine_test_kwargs
            )

        windows_cross_test_kwargs = {
            "size": _test_size(test_sizes, windows_cross_test_name),
        }
        windows_cross_test_kwargs.update(integration_test_kwargs)
        if windows_cross_test_shard_count:
            windows_cross_test_kwargs["shard_count"] = windows_cross_test_shard_count
            windows_cross_test_kwargs["flaky"] = True

        rust_test(
            name = windows_cross_test_binary,
            crate_name = test_crate_name,
            crate_root = test,
            srcs = [test],
            data = native.glob(["tests/**"], allow_empty = True) + integration_test_binaries + test_data_extra,
            compile_data = native.glob(["tests/**"], allow_empty = True) + integration_compile_data_extra,
            deps = all_crate_deps(normal = True, normal_dev = True) + maybe_deps + deps_extra,
            rustc_flags = rustc_flags_extra + WINDOWS_RUSTC_LINK_FLAGS + [
                "--remap-path-prefix=../codex-rs=",
                "--remap-path-prefix=codex-rs=",
            ],
            rustc_env = rustc_env,
            env = integration_test_cargo_env,
            target_compatible_with = WINDOWS_GNULLVM_ONLY,
            tags = test_tags + ["manual"],
        )

        workspace_root_test(
            name = windows_cross_test_name,
            chdir_workspace_root = False,
            env = integration_test_cargo_env,
            runfile_env = integration_test_cargo_env_runfiles,
            test_bin = ":" + windows_cross_test_binary,
            workspace_root_marker = "//codex-rs/utils/cargo-bin:repo_root.marker",
            target_compatible_with = WINDOWS_GNULLVM_ONLY,
            tags = test_tags,
            **windows_cross_test_kwargs
        )

    unknown_test_sizes = sorted([
        test_name
        for test_name in test_sizes
        if test_name not in generated_test_names
    ])
    if unknown_test_sizes:
        fail("test_sizes contains unknown generated test targets: {}".format(", ".join(unknown_test_sizes)))

    unknown_test_shard_counts = sorted([
        test_name
        for test_name in test_shard_counts
        if test_name not in generated_test_names
    ])
    if unknown_test_shard_counts:
        fail("test_shard_counts contains unknown generated test targets: {}".format(", ".join(unknown_test_shard_counts)))

def _test_shard_count(test_shard_counts, test_name):
    shard_count = test_shard_counts.get(test_name)
    if shard_count == None:
        return None

    if shard_count < 1:
        fail("test_shard_counts[{}] must be a positive integer".format(test_name))

    return shard_count

def _test_size(test_sizes, test_name):
    size = test_sizes.get(test_name, "small")
    if size not in ["small", "medium", "large", "enormous"]:
        fail("test_sizes[{}] must be a valid Bazel test size".format(test_name))

    return size
