"""Supply libsqlite3-sys's bindings and Bazel-owned native library."""

load("@rules_cc//cc:defs.bzl", "CcInfo")
load("@rules_rust//rust/private:providers.bzl", "BuildInfo")

def _sqlite_build_info_impl(ctx):
    out_dir = ctx.file.out_dir
    if not out_dir.is_directory:
        fail("out_dir must be a directory")

    return [
        BuildInfo(
            compile_data = depset(),
            dep_env = None,
            flags = None,
            linker_flags = None,
            link_search_paths = None,
            out_dir = out_dir,
            rustc_env = None,
        ),
        ctx.attr.cc_lib[CcInfo],
    ]

sqlite_build_info = rule(
    implementation = _sqlite_build_info_impl,
    attrs = {
        "cc_lib": attr.label(mandatory = True, providers = [CcInfo]),
        "out_dir": attr.label(allow_single_file = True, mandatory = True),
    },
)
