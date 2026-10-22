"""Source-only, private Abseil build for V8.

Abseil's inline namespace is deliberately set in options.h, not a command line
override: upstream's options header defines it unconditionally. That file is
Abseil's supported build configuration interface for packagers. Namespace all
of the V8 copy (including its public headers) identically so it can coexist in
the same binary with the shared Abseil module compiled for protobuf.
"""

def _v8_abseil_repository_impl(repository_ctx):
    repository_ctx.download_and_extract(
        url = "https://github.com/abseil/abseil-cpp/releases/download/20250814.1/abseil-cpp-20250814.1.tar.gz",
        integrity = "sha256-FpL3fRc5us8/lDNxiLeFg88JurfkINLcbFYFpPhnhaE=",
        stripPrefix = "abseil-cpp-20250814.1",
    )

    # Our root BUILD must own all sources beneath absl/. Native Starlark
    # filesystem operations work on Windows as well; no shell or Python.
    directories = [repository_ctx.path("absl")]
    for _ in range(32):
        if not directories:
            break
        next_directories = []
        for directory in directories:
            for entry in directory.readdir():
                if entry.basename in ["BUILD", "BUILD.bazel"]:
                    repository_ctx.delete(entry)
                elif entry.is_dir:
                    next_directories.append(entry)
        directories = next_directories
    if directories:
        fail("V8 Abseil source tree exceeds the supported traversal depth")
    repository_ctx.delete("BUILD")
    repository_ctx.delete("BUILD.bazel")
    repository_ctx.symlink(repository_ctx.attr.build_file, "BUILD.bazel")

    # Do not silently configure a new Abseil version as this version on upgrade.
    # See absl/base/options.h: ABSL_OPTION_INLINE_NAMESPACE_NAME.
    options = repository_ctx.read("absl/base/options.h")
    original = "#define ABSL_OPTION_INLINE_NAMESPACE_NAME lts_20250814"
    if options.count(original) != 1:
        fail("Unexpected Abseil inline namespace: recheck the private V8 ABI for this source revision")
    repository_ctx.file(
        "absl/base/options.h",
        options.replace(original, "#define ABSL_OPTION_INLINE_NAMESPACE_NAME codex_v8_lts_20250814"),
        executable = False,
    )

v8_abseil_repository = repository_rule(
    implementation = _v8_abseil_repository_impl,
    attrs = {
        "build_file": attr.label(
            default = Label("//third_party/v8:abseil.BUILD.bazel"),
            allow_single_file = True,
        ),
    },
)
