"""Exposes a fixed hermetic Python forwarder to Bazel tests."""

HermeticTestPythonInfo = provider(
    doc = "The fixed forwarding executable backed by the hermetic target Python toolchain.",
    fields = {
        "executable": "The main-repository Python forwarding executable.",
    },
)

def _hermetic_test_python_impl(ctx):
    launcher_info = ctx.attr._launcher[DefaultInfo]
    files_to_run = launcher_info.files_to_run
    executable = files_to_run.executable
    if executable == None:
        fail("the hermetic Python forwarder must provide an executable")

    support_files = [executable]
    if files_to_run.runfiles_manifest != None:
        support_files.append(files_to_run.runfiles_manifest)
    if files_to_run.repo_mapping_manifest != None:
        support_files.append(files_to_run.repo_mapping_manifest)

    return [
        DefaultInfo(
            files = launcher_info.files,
            runfiles = ctx.runfiles(
                files = support_files,
                transitive_files = launcher_info.files,
            ).merge(launcher_info.default_runfiles),
        ),
        HermeticTestPythonInfo(executable = executable),
    ]

hermetic_test_python = rule(
    implementation = _hermetic_test_python_impl,
    attrs = {
        "_launcher": attr.label(
            cfg = "target",
            default = "//bazel/rules/testing:_hermetic_test_python",
            executable = True,
        ),
    },
)
