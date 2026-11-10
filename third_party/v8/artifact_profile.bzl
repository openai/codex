"""Complete V8 artifact profiles, enforced at the source archive boundary.

Native Windows builds embed Chromium libc++ with the consumer's stdio ABI.
Other source profiles use the workspace C++ runtime. Published profiles embed
the pinned Chromium runtime. Generators inherit the
V8 target CPU even when their executable is built for the exec platform.
"""

_POINTER_COMPRESSION = "@v8//:v8_enable_pointer_compression"
_SANDBOX = "@v8//:v8_enable_sandbox"
_LIBCXX = "@v8//:v8_use_rusty_v8_custom_libcxx"
_TARGET_CPU = "@v8//bazel/config:v8_target_cpu"
_TARGET_OS = "@v8//:v8_target_os"
_MSVC_PROFILE = "@v8//:v8_windows_msvc_profile"

# The published MSVC archives use the Windows SDK's native wide-stdio mode.
# Apply this after clang-cl's default ISO define to the entire archive graph.
_MSVC_NATIVE_STDIO_COPT = "-U_CRT_STDIO_ISO_WIDE_SPECIFIERS"

_COPT = "//command_line_option:copt"
_NOSTDINCXX = "@llvm//config:nostdincxx"
_NOSTDLIBXX = "@llvm//config:nostdlibxx"
_CPU_CONSTRAINTS = {
    "arm64": Label("@platforms//cpu:aarch64"),
    "x64": Label("@platforms//cpu:x86_64"),
}

def _artifact_profile_impl(settings, attr):
    sandbox = attr.profile != "release"
    use_chromium_libcxx = attr.native_windows or attr.profile != "source"
    copts = settings[_COPT]
    if attr.native_windows and attr.profile != "source":
        copts = copts + [_MSVC_NATIVE_STDIO_COPT]
    return {
        _COPT: copts,
        _POINTER_COMPRESSION: "True" if sandbox else "False",
        _SANDBOX: sandbox,
        _LIBCXX: use_chromium_libcxx,
        _TARGET_CPU: attr.v8_cpu,
        # Like GN's target_os, these settings survive the exec transition.
        # Windows snapshot generators run under Wine on Linux.
        _TARGET_OS: attr.target_os,
        _MSVC_PROFILE: attr.native_windows,
        _NOSTDINCXX: True if use_chromium_libcxx else settings[_NOSTDINCXX],
        _NOSTDLIBXX: True if attr.native_windows else settings[_NOSTDLIBXX],
    }

_artifact_profile = transition(
    implementation = _artifact_profile_impl,
    inputs = [_COPT, _NOSTDINCXX, _NOSTDLIBXX],
    outputs = [_COPT, _POINTER_COMPRESSION, _SANDBOX, _LIBCXX, _TARGET_CPU, _TARGET_OS, _MSVC_PROFILE, _NOSTDINCXX, _NOSTDLIBXX],
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
        "target_os": attr.string(mandatory = True),
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
        target_os = select({
            "@platforms//os:windows": "win",
            "@platforms//os:macos": "mac",
            "@platforms//os:linux": "linux",
            "@platforms//os:android": "android",
            "//conditions:default": "",
        }),
        v8_cpu = v8_cpu,
        **kwargs
    )
