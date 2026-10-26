"""Complete V8 artifact profiles, enforced at the source archive boundary.

The source profile is for local consumers and uses the workspace C++ runtime.
Published profiles embed the pinned Chromium runtime. Generators inherit the
V8 target CPU even when their executable is built for the exec platform.
"""

_POINTER_COMPRESSION = "@v8//:v8_enable_pointer_compression"
_SANDBOX = "@v8//:v8_enable_sandbox"
_LIBCXX = "@v8//:v8_use_rusty_v8_custom_libcxx"
_TARGET_CPU = "@v8//bazel/config:v8_target_cpu"
_TOOLCHAINS = "//command_line_option:extra_toolchains"
_NATIVE_WINDOWS_TOOLCHAINS = [
    str(Label("@v8_native_windows_cc//:cc-toolchain-x64_windows")),
    str(Label("@v8_native_windows_cc//:cc-toolchain-arm64_windows")),
]
_CPU_CONSTRAINTS = {
    "arm64": Label("@platforms//cpu:aarch64"),
    "x64": Label("@platforms//cpu:x86_64"),
}

def _artifact_profile_impl(settings, attr):
    sandbox = attr.profile != "release"
    return {
        _POINTER_COMPRESSION: "True" if sandbox else "False",
        _SANDBOX: sandbox,
        _LIBCXX: attr.profile != "source",
        _TARGET_CPU: attr.v8_cpu,
        # Select clang-cl only below the V8 archive/binding boundary. Keep the
        # caller's toolchains (including the ordinary MSVC compiler) elsewhere.
        _TOOLCHAINS: settings[_TOOLCHAINS] + (_NATIVE_WINDOWS_TOOLCHAINS if attr.native_windows else []),
    }

_artifact_profile = transition(
    implementation = _artifact_profile_impl,
    inputs = [_TOOLCHAINS],
    outputs = [_POINTER_COMPRESSION, _SANDBOX, _LIBCXX, _TARGET_CPU, _TOOLCHAINS],
)

def _profiled_files_impl(ctx):
    constraint_target = ctx.attr._arm64 if ctx.attr.v8_cpu == "arm64" else ctx.attr._x64
    if not ctx.target_platform_has_constraint(constraint_target[platform_common.ConstraintValueInfo]):
        fail("%s: V8 CPU %s requires %s on the Bazel target platform. Choose a matching artifact target and --platforms." % (
            ctx.label,
            ctx.attr.v8_cpu,
            constraint_target.label,
        ))
    files = depset(transitive = [target[DefaultInfo].files for target in ctx.attr.srcs])
    runfiles = ctx.runfiles().merge_all([
        target[DefaultInfo].default_runfiles
        for target in ctx.attr.srcs
    ])
    return [DefaultInfo(files = files, runfiles = runfiles)]

_v8_artifact_files = rule(
    implementation = _profiled_files_impl,
    doc = "Build a source archive or release pair under its named V8 profile.",
    attrs = {
        "native_windows": attr.bool(mandatory = True),
        "profile": attr.string(
            mandatory = True,
            values = ["source", "release", "sandbox_release"],
        ),
        "srcs": attr.label_list(allow_files = True, cfg = _artifact_profile, mandatory = True),
        "v8_cpu": attr.string(mandatory = True, values = ["arm64", "x64"]),
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
        "_arm64": attr.label(default = _CPU_CONSTRAINTS["arm64"]),
        "_x64": attr.label(default = _CPU_CONSTRAINTS["x64"]),
    },
)

def v8_artifact_files(name, v8_cpu, **kwargs):
    """Declare an artifact that is only valid for its requested CPU.

    Fixed-CPU targets without explicit platform constraints can be skipped in
    wildcard builds. Preserve constraints on targets such as native Windows
    which already specify their CPU and ABI. The rule checks the resolved CPU
    in both cases, including values chosen by select().
    The generator exec platform remains independent of this target constraint.
    """
    if type(v8_cpu) == "string":
        kwargs.setdefault("target_compatible_with", [_CPU_CONSTRAINTS[v8_cpu]])
    _v8_artifact_files(
        name = name,
        # Derived from the target platform; callers cannot set this knob.
        native_windows = select({
            "@rules_rs//rs/platforms/config:aarch64-pc-windows-msvc": True,
            "@rules_rs//rs/platforms/config:x86_64-pc-windows-msvc": True,
            "//conditions:default": False,
        }),
        v8_cpu = v8_cpu,
        **kwargs
    )
