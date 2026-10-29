"""Generate API artifacts with the existing Rust and Python exporters."""

load("@bazel_lib//lib:copy_file.bzl", "COPY_FILE_TOOLCHAINS", "copy_file_action")

def _schema_bundle_impl(ctx):
    outputs = []
    for mode in ["stable", "experimental"]:
        schema = ctx.actions.declare_directory(ctx.label.name + "." + mode)
        ctx.actions.run(
            executable = ctx.executable.generator,
            tools = [ctx.attr.generator[DefaultInfo].files_to_run],
            env = {
                "CODEX_APP_SERVER_SCHEMA_ROOT": schema.path,
                "CODEX_APP_SERVER_SCHEMA_EXPERIMENTAL": "1" if mode == "experimental" else "0",
            },
            outputs = [schema],
            mnemonic = "PublicSchema",
            progress_message = "Generating %s public app-server schemas" % mode,
        )
        outputs.append(schema)

    # Expose the pinned compressor alongside the cached directories so callers
    # can normalize the JSON without a separate `bazel run` invocation.
    zstd = ctx.actions.declare_file(ctx.label.name + ".zstd")
    ctx.actions.symlink(output = zstd, target_file = ctx.executable.zstd, is_executable = True)
    return [
        DefaultInfo(files = depset(outputs + [zstd])),
        OutputGroupInfo(stable = depset([outputs[0]]), experimental = depset([outputs[1]])),
    ]

schema_bundle = rule(
    implementation = _schema_bundle_impl,
    attrs = {
        "generator": attr.label(mandatory = True, executable = True, cfg = "exec"),
        "zstd": attr.label(default = "@zstd//:zstd_cli", executable = True, cfg = "exec"),
    },
    doc = "Generates both schema modes as cacheable actions with explicit tool dependencies.",
)

def _schema_sources_impl(ctx):
    sources = []
    crate_root = None
    prefix = ctx.label.package + "/"
    for source in ctx.files.srcs:
        if not source.short_path.startswith(prefix):
            fail("Schema generator sources must belong to " + ctx.label.package)
        relative = source.short_path[len(prefix):]
        output = ctx.actions.declare_file(ctx.label.name + "/" + relative)
        ctx.actions.symlink(output = output, target_file = source)
        sources.append(output)
        if relative == ctx.attr.crate_root:
            crate_root = output
    if crate_root == None:
        fail("Schema generator sources must include " + ctx.attr.crate_root)

    # Runtime builds embed generated bundles. The generator needs only empty
    # placeholders because it derives the bundles from Rust.
    precomputed = [
        ctx.actions.declare_file(ctx.label.name + "/schema/precomputed/app-server-exports-" + mode + ".json.zst")
        for mode in ["stable", "experimental"]
    ]
    for mode, output in zip(["stable", "experimental"], precomputed):
        if ctx.attr.schema:
            directory = getattr(ctx.attr.schema[OutputGroupInfo], mode).to_list()[0]
            copy_file_action(ctx, directory, output, dir_path = "precomputed/" + output.basename)
        else:
            ctx.actions.write(output, "")
    return [
        DefaultInfo(files = depset(sources)),
        OutputGroupInfo(crate_root = depset([crate_root]), precomputed = depset(precomputed)),
    ]

schema_sources = rule(
    implementation = _schema_sources_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = [".rs"]),
        "crate_root": attr.string(default = "src/lib.rs"),
        "schema": attr.label(providers = [OutputGroupInfo]),
    },
    toolchains = COPY_FILE_TOOLCHAINS,
)

def _python_sdk_exports_impl(ctx):
    output = ctx.actions.declare_directory(ctx.label.name)
    manifest = ctx.actions.declare_file(ctx.label.name + ".json")
    prefix = ctx.label.package + "/"
    files = []
    for source in ctx.files.srcs:
        if not source.short_path.startswith(prefix):
            fail("Python SDK generator inputs must belong to " + ctx.label.package)
        files.append([source.path, source.short_path[len(prefix):]])
    ctx.actions.write(manifest, json.encode(files))
    ctx.actions.run(
        executable = ctx.executable._generator,
        arguments = [manifest.path, ctx.file.schema.path + "/json", output.path],
        inputs = ctx.files.srcs + [ctx.file.schema, manifest],
        tools = [ctx.attr._generator[DefaultInfo].files_to_run],
        outputs = [output],
        env = {"PYTHONDONTWRITEBYTECODE": "1", "PYTHONUTF8": "1"},
        mnemonic = "PythonSdkExports",
    )
    return [DefaultInfo(files = depset([output]))]

python_sdk_exports = rule(
    implementation = _python_sdk_exports_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "schema": attr.label(mandatory = True, allow_single_file = True),
        "_generator": attr.label(
            default = "//bazel/schema/tools:python-codegen",
            executable = True,
            cfg = "exec",
        ),
    },
)
