"""Exposes the selected hermetic target Python runtime to Bazel tests."""

HermeticTestPythonInfo = provider(
    doc = "The interpreter selected from the hermetic target Python toolchain.",
    fields = {
        "interpreter": "The in-build Python interpreter file.",
    },
)

def _hermetic_test_python_impl(ctx):
    runtime = ctx.toolchains["@rules_python//python:toolchain_type"].py3_runtime
    if runtime == None or runtime.interpreter == None:
        fail("the hermetic Python toolchain must provide an in-build interpreter")

    runtime_files = depset(
        direct = [runtime.interpreter],
        transitive = [runtime.files],
    )
    return [
        DefaultInfo(
            files = runtime_files,
            runfiles = ctx.runfiles(transitive_files = runtime_files),
        ),
        HermeticTestPythonInfo(interpreter = runtime.interpreter),
    ]

hermetic_test_python = rule(
    implementation = _hermetic_test_python_impl,
    toolchains = ["@rules_python//python:toolchain_type"],
)
