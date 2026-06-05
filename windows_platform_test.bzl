def _windows_platform_declarations_test_impl(ctx):
    executable = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.symlink(
        output = executable,
        target_file = ctx.executable.script,
        is_executable = True,
    )
    return [
        DefaultInfo(
            executable = executable,
            runfiles = ctx.runfiles(files = ctx.files.data),
        ),
    ]

windows_platform_declarations_test = rule(
    implementation = _windows_platform_declarations_test_impl,
    attrs = {
        "data": attr.label_list(allow_files = True),
        "script": attr.label(
            allow_single_file = True,
            cfg = "target",
            executable = True,
            mandatory = True,
        ),
    },
    test = True,
)
