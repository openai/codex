"""Inputs for the native Windows V8 build, using rules_cc's installed SDK."""

load("@llvm//toolchain:selects.bzl", "LLVM_VERSION")
load("@rules_cc//cc/private/toolchain:cc_configure.bzl", "MSVC_ENVVARS")
load("@rules_cc//cc/private/toolchain:windows_cc_configure.bzl", "configure_windows_toolchain")

# Match @llvm 0.8.11's bindgen compiler. Unlike its minimal bootstrap seed,
# these official distributions also contain the MSVC compiler-rt builtins.
_LLVM_WINDOWS = {
    "x86_64": "d96c2cc1736f4eb7fa43cb9bbdf56d93551a9ae0a9aadb9c99c3c3b2b712a234",
    "aarch64": "de718c58ebbc5f61d58c17b90457fcf42983bc2c4a4aba3e010d108713bfd7f1",
}

def _native_windows_llvm_impl(repository_ctx):
    if repository_ctx.os.name.lower().find("windows") != -1:
        arch = repository_ctx.os.arch.lower()
        arch = {"amd64": "x86_64", "arm64": "aarch64"}.get(arch, arch)
        if arch not in _LLVM_WINDOWS:
            fail("V8's native Windows build supports x64 and arm64, got " + arch)
        filename = "clang+llvm-%s-%s-pc-windows-msvc" % (LLVM_VERSION, arch)
        repository_ctx.download_and_extract(
            url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-%s/%s.tar.xz" % (LLVM_VERSION, filename.replace("+", "%2B")),
            sha256 = _LLVM_WINDOWS[arch],
            stripPrefix = filename,
        )

    # Native-only inputs are empty on other hosts, without fetching Windows
    # archives on Linux/macOS during //... queries or public lockfile checks.
    repository_ctx.file("BUILD.bazel", """
load("@rules_cc//cc:defs.bzl", "cc_import")
package(default_visibility = ["//visibility:public"])
exports_files(["BUILD.bazel"])
filegroup(name = "llvm-readobj", srcs = glob(["bin/llvm-readobj.exe"], allow_empty = True))
filegroup(name = "llvm-lib", srcs = glob(["bin/llvm-lib.exe"], allow_empty = True))
# Tool paths alone do not invalidate cached C++ actions when the pin changes.
# Keep compiler resources and bundled runtime DLLs in the toolchain inputs.
filegroup(
    name = "compiler_files",
    srcs = glob(["bin/clang-cl.exe", "bin/*.dll", "lib/clang/*/include/**"], allow_empty = True),
)
filegroup(
    name = "linker_files",
    srcs = glob(["bin/lld-link.exe", "bin/*.dll"], allow_empty = True),
)
filegroup(
    name = "archiver_files",
    srcs = glob(["bin/llvm-lib.exe", "bin/*.dll"], allow_empty = True),
)
filegroup(
    name = "all_files",
    srcs = [":compiler_files", ":linker_files", ":archiver_files"],
)
[
    filegroup(
        name = "builtins_" + arch,
        srcs = glob(["lib/clang/*/lib/windows/clang_rt.builtins-%s.lib" % arch], allow_empty = True),
    )
    for arch in ["x86_64", "aarch64"]
]
filegroup(
    name = "builtins_archive",
    srcs = select({
        "@platforms//cpu:x86_64": [":builtins_x86_64"],
        "@platforms//cpu:aarch64": [":builtins_aarch64"],
    }),
)
cc_import(
    name = "builtins",
    static_library = ":builtins_archive",
)
""")

native_windows_llvm = repository_rule(
    implementation = _native_windows_llvm_impl,
)

def _native_windows_cc_impl(repository_ctx):
    if repository_ctx.os.name.lower().find("windows") != -1:
        configure_windows_toolchain(repository_ctx)
    else:
        repository_ctx.file("BUILD.bazel", "# Native Windows toolchains are only available on Windows.\n")

native_windows_cc = repository_rule(
    implementation = _native_windows_cc_impl,
    attrs = {
        # A Bazel repo label rather than a user's ambient installation path.
        # The rules_cc pin patch reads this only for this opt-in repository.
        "native_llvm": attr.label(allow_single_file = True, mandatory = True),
    },
    environ = MSVC_ENVVARS + [
        "BAZEL_CONLYOPTS",
        "BAZEL_COPTS",
        "BAZEL_CXXOPTS",
        "BAZEL_LINKOPTS",
        "BAZEL_SH",
        "BAZEL_WIN32_WINNT",
        "SYSTEMROOT",
        "PROGRAMFILES",
    ],
    configure = True,
    local = True,
)
