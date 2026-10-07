"""Build-only native voice prefix using the same recipe as release packaging."""

load("@bazel_tools//tools/cpp:toolchain_utils.bzl", "find_cpp_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_foreign_cc//foreign_cc/private:cc_toolchain_util.bzl", "absolutize_path_in_str", "get_env_vars", "get_flags_info", "get_tools_info")
load("@rules_python//python:py_runtime_info.bzl", "PyRuntimeInfo")

def _native_prefix_impl(ctx):
    cc = find_cpp_toolchain(ctx)
    features = cc_common.configure_features(
        ctx = ctx,
        cc_toolchain = cc,
        requested_features = ctx.features,
        unsupported_features = ctx.disabled_features,
    )
    runtime = cc.static_runtime_lib(feature_configuration = features)
    tools = get_tools_info(ctx)
    flags = get_flags_info(ctx)
    minimums = [flag.removeprefix("-mmacosx-version-min=") for flag in flags.cc if flag.startswith("-mmacosx-version-min=")]
    if ctx.attr.target.endswith("apple-darwin") and len(minimums) != 1:
        fail("The native prefix requires one macOS minimum from the CcToolchain")
    python = ctx.attr._python[PyRuntimeInfo]
    if not python.interpreter:
        fail("The native prefix requires a declared Python interpreter")
    prefix = ctx.actions.declare_file(ctx.label.name + "/prefix.tar")
    receipt = ctx.actions.declare_file(ctx.label.name + "/built.json")
    config = ctx.actions.declare_file(ctx.label.name + ".json")
    values = {
        "archives": [file.path for file in ctx.files.archives],
        "prefix": prefix.path,
        "receipt": receipt.path,
        "target": ctx.attr.target,
        "deployment_target": minimums[0] if minimums else None,
        "jobs": 8,
        "cc": tools.cc,
        "cxx": tools.cxx,
        "ar": tools.cxx_linker_static,
        "ranlib": ctx.executable._ranlib.path,
        "ld": ctx.executable._ld.path,
        "shell": "/bin/bash",
        "pkg_config": ctx.file._pkg_config.path,
    }
    inputs = [cc.all_files, python.files, runtime, ctx.attr._pkg_config.files]
    for name in ("cmake", "make"):
        tool = ctx.toolchains["@rules_foreign_cc//toolchains:" + name + "_toolchain"].data
        if not tool.target:
            fail("The native prefix requires a declared " + name + " tool")
        values[name] = tool.path

        # Prebuilt tools expose a package-relative path; built tools already
        # include their tree artifact. Match the upstream tool-access convention.
        for file in tool.target.files.to_list():
            if file.path.endswith("/" + tool.path):
                values[name] = file.path
                break
        inputs.append(tool.target.files)
    for name, arguments in {
        "c_flag": flags.cc,
        "cxx_flag": flags.cxx,
        # Upstream selects executable vs shared output. Retain their common
        # driver/SDK flags without forcing one output kind on configure probes.
        "link_flag": [flag for flag in flags.cxx_linker_shared if flag in flags.cxx_linker_executable] + [file.path for file in runtime.to_list()],
    }.items():
        values[name] = [absolutize_path_in_str(ctx.workspace_name, "@VOICE_EXECROOT@/", flag) for flag in arguments]
    ctx.actions.write(config, json.encode(values))
    ctx.actions.run(
        executable = python.interpreter,
        arguments = [ctx.file._driver.path, config.path],
        inputs = depset(
            [config, ctx.file._driver, python.interpreter] + ctx.files.archives + ctx.files._recipe,
            transitive = inputs,
        ),
        tools = [
            ctx.attr._ranlib[DefaultInfo].files_to_run,
            ctx.attr._ld[DefaultInfo].files_to_run,
        ],
        outputs = [prefix, receipt],
        env = get_env_vars(ctx),
        execution_requirements = {
            "no-remote-exec": "1",
            "no-remote-cache": "1",
        } if ctx.attr.target.endswith("apple-darwin") else {},
        mnemonic = "VoiceNativePrefix",
        progress_message = "Building native voice prefix for " + ctx.attr.target,
    )
    return [
        DefaultInfo(files = depset([prefix])),
        OutputGroupInfo(receipt = depset([receipt])),
    ]

native_prefix = rule(
    implementation = _native_prefix_impl,
    attrs = {
        "archives": attr.label_list(allow_files = True, mandatory = True),
        "copts": attr.string_list(),
        "target": attr.string(mandatory = True),
        "_driver": attr.label(default = "//third_party/voice:bazel_native.py", allow_single_file = True),
        "_recipe": attr.label(default = "//third_party/voice:native_recipe"),
        "_python": attr.label(default = "@python_3_12//:py3_runtime", cfg = "exec"),
        "_ranlib": attr.label(default = "@llvm//tools:llvm-ranlib", executable = True, allow_files = True, cfg = "exec"),
        "_ld": attr.label(default = "//third_party/voice:pkg_config_linker", executable = True, allow_files = True, cfg = "exec"),
        "_pkg_config": attr.label(default = "//third_party/voice:pkg_config", allow_single_file = True, cfg = "exec"),
    },
    fragments = ["cpp"],
    toolchains = [
        "@bazel_tools//tools/cpp:toolchain_type",
        "@rules_foreign_cc//toolchains:cmake_toolchain",
        "@rules_foreign_cc//toolchains:make_toolchain",
    ],
)
