"""Rules for building release binaries across supported target platforms."""

_PLATFORMS = {
    "linux_arm64_musl": struct(
        platform = "//bazel/platforms:linux_arm64_musl",
        v8_target_cpu = "arm64",
    ),
    "linux_amd64_musl": struct(
        platform = "//bazel/platforms:linux_amd64_musl",
        v8_target_cpu = "x64",
    ),
    "macos_amd64": struct(
        platform = "@llvm//platforms:macos_amd64",
        v8_target_cpu = "x64",
    ),
    "macos_arm64": struct(
        platform = "@llvm//platforms:macos_arm64",
        v8_target_cpu = "arm64",
    ),
    "windows_amd64": struct(
        platform = "//bazel/platforms:windows_amd64",
        v8_target_cpu = "x64",
    ),
    "windows_arm64": struct(
        platform = "//bazel/platforms:windows_arm64",
        v8_target_cpu = "arm64",
    ),
}

def _release_binary_transition_impl(_settings, attr):
    return {
        "//command_line_option:platforms": str(attr.platform),
        "@v8//bazel/config:v8_target_cpu": attr.v8_target_cpu,
    }

_release_binary_transition = transition(
    implementation = _release_binary_transition_impl,
    inputs = [],
    outputs = [
        "//command_line_option:platforms",
        "@v8//bazel/config:v8_target_cpu",
    ],
)

def _release_binary_impl(ctx):
    target = ctx.attr.target[0][DefaultInfo]
    original_executable = target.files_to_run.executable
    if original_executable == None:
        fail("{} does not provide an executable".format(ctx.attr.target[0].label))

    executable = ctx.actions.declare_file(ctx.attr.name)
    ctx.actions.symlink(
        output = executable,
        target_file = original_executable,
        is_executable = True,
    )

    return [
        DefaultInfo(
            executable = executable,
            files = depset(
                direct = [executable],
                transitive = [target.files],
            ),
            runfiles = target.default_runfiles.merge(ctx.runfiles([executable])),
        ),
    ]

_release_binary = rule(
    implementation = _release_binary_impl,
    attrs = {
        "platform": attr.label(mandatory = True),
        "target": attr.label(
            allow_files = False,
            cfg = _release_binary_transition,
            executable = True,
            mandatory = True,
        ),
        "v8_target_cpu": attr.string(mandatory = True),
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
    },
    executable = True,
)

def multiplatform_binaries(name, platforms = _PLATFORMS):
    for platform_name, config in platforms.items():
        _release_binary(
            name = name + "_" + platform_name,
            platform = config.platform,
            target = name,
            tags = ["manual"],
            v8_target_cpu = config.v8_target_cpu,
        )

    native.filegroup(
        name = "release_binaries",
        srcs = [name + "_" + platform for platform in platforms.keys()],
        tags = ["manual"],
    )
