"""Makes a binary built for a foreign platform available as test data."""

_EXTRA_RUSTC_FLAGS = "@rules_rust//rust/settings:extra_rustc_flags"

def _foreign_platform_transition_impl(settings, attr):
    # A transition cannot rewrite a dependency's rule attributes. Use the
    # rules_rust build setting when every Rust target in the foreign
    # configuration needs additional compiler or linker flags.
    return {
        "//command_line_option:platforms": [attr.platform],
        _EXTRA_RUSTC_FLAGS: settings[_EXTRA_RUSTC_FLAGS] + attr.extra_rustc_flags,
    }

_foreign_platform_transition = transition(
    implementation = _foreign_platform_transition_impl,
    inputs = [_EXTRA_RUSTC_FLAGS],
    outputs = [
        "//command_line_option:platforms",
        _EXTRA_RUSTC_FLAGS,
    ],
)

def _foreign_platform_binary_impl(ctx):
    if len(ctx.attr.binary) != 1:
        fail("expected exactly one transitioned binary")
    binary = ctx.attr.binary[0][DefaultInfo]
    runfiles = ctx.runfiles(transitive_files = binary.files)
    runfiles = runfiles.merge(binary.default_runfiles)
    return [
        DefaultInfo(
            files = binary.files,
            runfiles = runfiles,
        ),
    ]

foreign_platform_binary = rule(
    implementation = _foreign_platform_binary_impl,
    attrs = {
        "binary": attr.label(
            cfg = _foreign_platform_transition,
            executable = True,
            mandatory = True,
        ),
        "extra_rustc_flags": attr.string_list(
            doc = "Additional flags applied to every Rust target in the foreign configuration.",
        ),
        "platform": attr.string(mandatory = True),
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
    },
    doc = "Builds `binary` for `platform` and exposes its files and runfiles.",
)
