"""Bazel aspect for running argument-comment-lint on Rust targets."""

load("@rules_rust//rust:defs.bzl", "rust_common")
load("@rules_rust//rust/private:rust.bzl", "RUSTC_ATTRS")
load(
    "@rules_rust//rust/private:rustc.bzl",
    "collect_deps",
    "collect_inputs",
    "construct_arguments",
)
load(
    "@rules_rust//rust/private:utils.bzl",
    "determine_output_hash",
    "find_cc_toolchain",
    "find_toolchain",
)

_STRICT_LINT_FLAGS = [
    "-Dargument-comment-mismatch",
    "-Duncommented-anonymous-literal-argument",
    "-Aunknown-lints",
]

def _find_rustc_driver_library(toolchain):
    for file in toolchain.rustc_lib.to_list():
        if file.basename.startswith("librustc_driver-") or file.basename.startswith("rustc_driver-"):
            return file
    return None

def _prepend_runtime_path(env, key, path, separator):
    previous = env.get(key)
    env[key] = "{}{}{}".format(path, separator, previous) if previous else path

def _set_driver_runtime_env(env, toolchain):
    driver_library = _find_rustc_driver_library(toolchain)
    if not driver_library:
        return

    library_dir = driver_library.dirname
    if driver_library.basename.endswith(".dll"):
        _prepend_runtime_path(env, "PATH", library_dir, ";")
        return

    # The lint driver runs in exec configuration. Under remote execution the
    # exec OS can differ from the Rust target OS, so populate both Unix loader
    # variables from the located driver library instead of keying off target_os.
    _prepend_runtime_path(env, "LD_LIBRARY_PATH", library_dir, ":")
    _prepend_runtime_path(env, "DYLD_LIBRARY_PATH", library_dir, ":")

def _get_argument_comment_lint_ready_crate_info(target, aspect_ctx):
    if target.label.workspace_root.startswith("external"):
        return None

    if aspect_ctx:
        ignore_tags = [
            "no_argument_comment_lint",
            "no-lint",
            "no_lint",
            "nolint",
        ]
        for tag in aspect_ctx.rule.attr.tags:
            if tag.replace("-", "_").lower() in ignore_tags:
                return None

    if rust_common.crate_info in target:
        return target[rust_common.crate_info]
    if rust_common.test_crate_info in target:
        return target[rust_common.test_crate_info].crate
    return None

def _rust_argument_comment_lint_aspect_impl(target, ctx):
    if OutputGroupInfo in target and hasattr(target[OutputGroupInfo], "argument_comment_lint_checks"):
        return []

    crate_info = _get_argument_comment_lint_ready_crate_info(target, ctx)
    if not crate_info:
        return []

    toolchain = find_toolchain(ctx)
    cc_toolchain, feature_configuration = find_cc_toolchain(ctx)

    dep_info, build_info, _ = collect_deps(
        deps = crate_info.deps.to_list(),
        proc_macro_deps = crate_info.proc_macro_deps.to_list(),
        aliases = crate_info.aliases,
    )

    compile_inputs, out_dir, build_env_files, build_flags_files, linkstamp_outs, ambiguous_libs = collect_inputs(
        ctx,
        ctx.rule.file,
        ctx.rule.files,
        depset([]),
        toolchain,
        cc_toolchain,
        feature_configuration,
        crate_info,
        dep_info,
        build_info,
        [],
    )

    success_marker = ctx.actions.declare_file(
        ctx.label.name + ".argument_comment_lint.ok",
        sibling = crate_info.output,
    )

    args, env = construct_arguments(
        ctx = ctx,
        attr = ctx.rule.attr,
        file = ctx.file,
        toolchain = toolchain,
        tool_path = ctx.executable._driver.path,
        cc_toolchain = cc_toolchain,
        feature_configuration = feature_configuration,
        crate_info = crate_info,
        dep_info = dep_info,
        linkstamp_outs = linkstamp_outs,
        ambiguous_libs = ambiguous_libs,
        output_hash = determine_output_hash(crate_info.root, ctx.label),
        rust_flags = [],
        out_dir = out_dir,
        build_env_files = build_env_files,
        build_flags_files = build_flags_files,
        emit = ["dep-info", "metadata"],
        skip_expanding_rustc_env = True,
    )

    if crate_info.is_test:
        args.rustc_flags.add("--test")

    args.process_wrapper_flags.add("--touch-file", success_marker)
    args.rustc_flags.add_all(_STRICT_LINT_FLAGS)
    _set_driver_runtime_env(env, toolchain)

    driver_runfiles = ctx.attr._driver[DefaultInfo].default_runfiles.files
    action_inputs = depset(
        transitive = [
            compile_inputs,
            driver_runfiles,
            toolchain.rustc_lib,
        ],
    )

    ctx.actions.run(
        executable = ctx.executable._process_wrapper,
        inputs = action_inputs,
        outputs = [success_marker],
        env = env,
        tools = [ctx.executable._driver],
        execution_requirements = {
            "no-sandbox": "1",
        },
        arguments = args.all,
        mnemonic = "ArgumentCommentLint",
        progress_message = "ArgumentCommentLint %{label}",
        toolchain = "@rules_rust//rust:toolchain_type",
    )

    return [OutputGroupInfo(argument_comment_lint_checks = depset([success_marker]))]

rust_argument_comment_lint_aspect = aspect(
    implementation = _rust_argument_comment_lint_aspect_impl,
    fragments = ["cpp"],
    attrs = {
        "_driver": attr.label(
            default = Label("//tools/argument-comment-lint:argument-comment-lint-driver"),
            executable = True,
            cfg = "exec",
        ),
    } | RUSTC_ATTRS,
    toolchains = [
        str(Label("@rules_rust//rust:toolchain_type")),
        config_common.toolchain_type("@bazel_tools//tools/cpp:toolchain_type", mandatory = False),
    ],
    required_providers = [
        [rust_common.crate_info],
        [rust_common.test_crate_info],
    ],
    doc = """\
Runs argument-comment-lint on Rust targets using Bazel's Rust dependency graph.

Example:

```output
$ bazel build --config=argument-comment-lint //codex-rs/...
```
""",
)
