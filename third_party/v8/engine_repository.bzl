"""V8 source import with Codex-owned Bazel entry points.

The root package is placed inside the fetched source tree so relative source
labels and globs have their normal Bazel semantics. Reuse upstream's platform
and CPU configuration in place, with Codex's narrow patch to bazel/defs.bzl
and the target CPU setting.
V8's MODULE and toolchain BUILD files are never part of Codex's dependency or
toolchain graph.
"""

# Upstream keeps its source groups in BUILD.bazel, which cannot be loaded as
# a Starlark library. Reuse filegroups, so labels/globs retain their semantics
# inside @v8. Compiled targets (including valid V8 v8_library/v8_binary calls)
# belong in engine.BUILD.bazel, where we wire Codex's dependencies and runtime.
_INVENTORY_SLOT = "# @CODEX_V8_UPSTREAM_FILEGROUPS@"
_FILE_GROUPS = "# File group rules"
_GENERATED_FILES = "# Generated files"

def _upstream_filegroups(upstream):
    before_and_groups = upstream.split(_FILE_GROUPS)
    if len(before_and_groups) != 2:
        fail("V8 source import: expected one File group rules section; review the new pin's BUILD.bazel")
    groups_and_after = before_and_groups[1].split(_GENERATED_FILES)
    if len(groups_and_after) != 2:
        fail("V8 source import: expected one Generated files boundary; review the new pin's BUILD.bazel")

    groups = []
    statement = []
    for line in groups_and_after[0].split("\n"):
        if statement:
            statement.append(line)
            if line == ")":
                source = "\n".join(statement)
                if statement[0] == "filegroup(":
                    groups.append(source)
                elif 'name = "lib_dragonbox",' not in source and 'name = "lib_fp16",' not in source:
                    fail("V8 filegroup import: unexpected library; review and wire it in third_party/v8/engine.BUILD.bazel: " + source)
                statement = []
        elif line in ["filegroup(", "v8_library("]:
            # The pinned section happens to contain two libraries. Both are
            # already provided by our dependency overlays; skip only those names.
            statement = [line]
        elif line.strip() and not line.startswith("#"):
            fail("V8 filegroup import: only filegroups can be reused here; review compiled targets (including v8_binary) in third_party/v8/engine.BUILD.bazel. Unexpected statement: " + line)

    # Group count and library order can change without changing the owned graph.
    # Fail only when the source section is empty, malformed, or unsafe to reuse.
    if statement:
        fail("V8 filegroup import: unterminated statement; review the pinned BUILD.bazel: " + "\n".join(statement))
    if not groups:
        fail("V8 filegroup import: no filegroups found; review the pinned BUILD.bazel")
    return "\n\n".join(groups)

def _engine_repository_impl(ctx):
    ctx.download_and_extract(
        url = ctx.attr.urls,
        integrity = ctx.attr.integrity,
        stripPrefix = ctx.attr.strip_prefix,
    )
    inventory = _upstream_filegroups(ctx.read("BUILD.bazel"))

    # MSVC and GNU Windows require different x64 stack scanners. Keep that one
    # choice in our graph while taking all other membership from the V8 pin.
    msvc_scanner = '"src/heap/base/asm/x64/push_registers_masm.asm"'
    if inventory.count(msvc_scanner) != 1:
        fail("V8 source import: expected one x64 MSVC scanner input; review the Windows stack selector")
    inventory = inventory.replace(msvc_scanner, '":windows_x64_stack_assembly"')
    template = ctx.read(ctx.attr._engine_build)
    if template.count(_INVENTORY_SLOT) != 1:
        fail("Codex V8 BUILD template must contain exactly one upstream filegroup slot")

    for patch in ctx.attr.patches:
        ctx.patch(patch, strip = ctx.attr.patch_strip)

    # Preserve V8's source layout and the two third-party package boundaries
    # (disarm and fadec); neither is in the Codex V8 closure. Keep upstream's
    # platform/CPU package and prevent upstream toolchain discovery.
    for path in [
        "MODULE.bazel",
        "BUILD.bazel",
        "bazel/toolchain/BUILD.bazel",
        "bazel/patches/BUILD.bazel",
    ]:
        ctx.delete(path)
    ctx.file("BUILD.bazel", template.replace(_INVENTORY_SLOT, inventory))

engine_repository = repository_rule(
    implementation = _engine_repository_impl,
    attrs = {
        "urls": attr.string_list(mandatory = True),
        "integrity": attr.string(mandatory = True),
        "strip_prefix": attr.string(mandatory = True),
        "patches": attr.label_list(allow_files = True),
        "patch_strip": attr.int(default = 3),
        "_engine_build": attr.label(
            default = Label("//third_party/v8:engine.BUILD.bazel"),
            allow_single_file = True,
        ),
    },
    doc = "Fetch the pinned V8 source tree and install Codex's engine graph.",
)
