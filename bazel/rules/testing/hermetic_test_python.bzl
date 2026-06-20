"""Exposes the fixed hermetic target Python runtime to Bazel tests."""

HermeticTestPythonInfo = provider(
    doc = "The fixed interpreter from the hermetic target Python toolchain.",
    fields = {
        "executable": "The hermetic target Python interpreter.",
    },
)

def _hermetic_test_python_impl(ctx):
    runtime = ctx.toolchains["@rules_python//python:toolchain_type"].py3_runtime
    if runtime == None:
        fail("the hermetic Python toolchain must provide a Python 3 runtime")

    executable = runtime.interpreter
    if executable == None:
        fail("the hermetic Python 3 runtime must provide an interpreter file")

    files = depset([executable], transitive = [runtime.files])

    return [
        DefaultInfo(
            files = files,
            runfiles = ctx.runfiles(transitive_files = files),
        ),
        HermeticTestPythonInfo(executable = executable),
    ]

hermetic_test_python = rule(
    implementation = _hermetic_test_python_impl,
    toolchains = ["@rules_python//python:toolchain_type"],
)
