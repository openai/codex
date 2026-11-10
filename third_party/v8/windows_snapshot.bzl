"""Run a Windows V8 snapshot generator natively or with declared Wine.

V8 15.2's Windows unwind writer is guarded by the generator's host OS. A Linux
generator with V8_TARGET_OS_WIN would silently omit embedded .pdata/.xdata.
The generator's Windows compilation platform is distinct from its executor.
"""

load("@rules_wine//wine:actions.bzl", "windows_actions")

def _windows_generator_platform_impl(settings, attr):
    # Preserve the target's build mode, features and V8 profile. Select the
    # executable matching the runner only after toolchain resolution. Only
    # that executable is an action input; the other variant is not built.
    return {
        arch: {
            # Keep the caller's platform identity when no CPU change is needed.
            # Distinct labels with the same platform name can otherwise produce
            # conflicting C++ actions under --experimental_platform_in_output_dir.
            "//command_line_option:platforms": settings["//command_line_option:platforms"] if arch == attr.platform_arch else [str(Label(platform))],
        }
        for arch, platform in {
            "x86_64": "@llvm//platforms:windows_x86_64_msvc",
            "aarch64": "@llvm//platforms:windows_aarch64_msvc",
        }.items()
    }

_windows_generator_platform = transition(
    implementation = _windows_generator_platform_impl,
    inputs = ["//command_line_option:platforms"],
    outputs = ["//command_line_option:platforms"],
)

def _windows_snapshot_impl(ctx):
    runner = windows_actions.resolve(ctx)

    # Wine's ARM64 distribution runs x64 generators through FEX. Native
    # Windows ARM64 uses an ARM64 generator for ARM64 snapshots; retain an
    # x64 generator for x64 snapshots because V8 has no x64 simulator.
    native_arm64 = runner.windows.backend == "native" and runner.windows.arch == "aarch64" and ctx.attr.target_arch == "arm64"
    generator_arch = "aarch64" if native_arm64 else "x86_64"
    generator = ctx.split_attr.tool[generator_arch][DefaultInfo].files_to_run
    outputs = [
        ctx.actions.declare_file(ctx.label.name + "/snapshot.cc"),
        ctx.actions.declare_file(ctx.label.name + "/embedded.S"),
    ]
    args = ctx.actions.args()
    args.add_all([
        "--embedded_variant=Default",
        "--target_os",
        "win",
        "--target_arch",
        ctx.attr.target_arch,
    ])
    args.add("--startup_src")
    windows_actions.add_path(runner, args, outputs[0])
    args.add("--embedded_src")
    windows_actions.add_path(runner, args, outputs[1])
    args.add_all(ctx.attr.args)
    windows_actions.run(
        ctx,
        runner = runner,
        executable = generator,
        arguments = [args],
        outputs = outputs,
        mnemonic = "V8WindowsSnapshot",
        progress_message = "Generating Windows V8 snapshot",
    )

    # Preserve the existing clang-cl assembly path, including Windows-native
    # toolchains that otherwise send generated .S files to MASM.
    embedded_cc = ctx.actions.declare_file(ctx.label.name + "/embedded.cc")
    ctx.actions.run(
        executable = ctx.attr.inline_asm_converter[DefaultInfo].files_to_run,
        inputs = [outputs[1]],
        outputs = [embedded_cc],
        arguments = [outputs[1].path, embedded_cc.path],
        mnemonic = "V8NativeWindowsInlineAssembly",
    )
    return [DefaultInfo(files = depset([outputs[0], embedded_cc]))]

def windows_snapshot_rule(cpu_transition):
    """Preserve V8's target CPU independently of the generator and its runner."""
    return rule(
        implementation = _windows_snapshot_impl,
        cfg = cpu_transition,
        attrs = {
            "args": attr.string_list(),
            "platform_arch": attr.string(mandatory = True, values = ["aarch64", "x86_64"]),
            "target_arch": attr.string(mandatory = True, values = ["arm64", "x64"]),
            "tool": attr.label(
                mandatory = True,
                cfg = _windows_generator_platform,
            ),
            "_allowlist_function_transition": attr.label(
                default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
            ),
            "inline_asm_converter": attr.label(executable = True, cfg = "exec", mandatory = True),
        },
        toolchains = [windows_actions.toolchain_type],
    )
