"""Archive unstripped release binaries and debug symbols for signing and packaging."""

load("@platforms//host:constraints.bzl", "HOST_CONSTRAINTS")
load("@rules_pkg//pkg:providers.bzl", "PackageFilegroupInfo", "PackageFilesInfo")
load("@rules_pkg//pkg:tar.bzl", "pkg_tar")

# Keep the profile in sync with --config=release in .bazelrc and Cargo.toml.
# Archive targets apply it themselves, including when built without CI flags.
_RUSTC_FLAGS = "@rules_rust//rust/settings:extra_rustc_flag"
_EXEC_RUSTC_FLAGS = "@rules_rust//rust/settings:extra_exec_rustc_flag"
_PROFILE_INPUTS = [
    "//command_line_option:features",
    "//command_line_option:host_features",
    "//command_line_option:copt",
    "//command_line_option:action_env",
    "//command_line_option:host_action_env",
    "//command_line_option:macos_minimum_os",
    "//command_line_option:host_platform",
    "//command_line_option:extra_toolchains",
    _RUSTC_FLAGS,
    _EXEC_RUSTC_FLAGS,
]
_PROFILE_OUTPUTS = _PROFILE_INPUTS + [
    "//command_line_option:compilation_mode",
    "@rules_rust//rust/settings:lto",
    "@rules_rust//rust/settings:codegen_units",
]

def _release_profile_impl(settings, attr):
    profile = {key: settings[key] for key in _PROFILE_INPUTS}
    smoke = settings["//codex-rs:runtime_package_build_mode"] == "debug"
    profile.update({
        "//command_line_option:compilation_mode": "opt",
        "@rules_rust//rust/settings:lto": "off" if smoke else "thin",
        "@rules_rust//rust/settings:codegen_units": 256 if smoke else 4,
        # opt mode selects opt-level=3, which disables assertions and overflow
        # checks by default. Unoptimized release smokes need the explicit opt-out.
        _RUSTC_FLAGS: settings[_RUSTC_FLAGS] + ([
            "-Copt-level=0",
            "-Cdebug-assertions=no",
        ] if smoke else []),
        # Cargo release build tools use opt-level=0, assertions/checks off,
        # and rustc's default 16 CGUs. Overflow checks follow debug assertions.
        _EXEC_RUSTC_FLAGS: settings[_EXEC_RUSTC_FLAGS] + [
            "-Copt-level=0",
            "-Ccodegen-units=16",
            "-Cdebug-assertions=no",
        ],
    })
    for key in ["//command_line_option:features", "//command_line_option:host_features"]:
        profile[key] = settings[key] + ["cargo-release-profile"]
    if attr.platform_os == "macos":
        profile[_RUSTC_FLAGS] += ["-Csplit-debuginfo=packed"]
        profile["//command_line_option:macos_minimum_os"] = attr.macos_minimum_os
        profile["//command_line_option:action_env"] += ["MACOSX_DEPLOYMENT_TARGET=" + attr.macos_minimum_os]

        # Host tools must run on the build host, including x86_64 cross builds.
        host_minimum_os = "11.0" if "@platforms//cpu:aarch64" in HOST_CONSTRAINTS else "10.12"
        profile["//command_line_option:host_action_env"] += ["MACOSX_DEPLOYMENT_TARGET=" + host_minimum_os]
    elif attr.platform_os == "linux":
        # Compatibility with the Cargo musl release: aws-lc jitter can hang.
        profile["//command_line_option:copt"] += ["-DDISABLE_CPU_JITTER_ENTROPY"]
    elif attr.platform_os == "windows" and "@platforms//os:windows" in HOST_CONSTRAINTS:
        # Native MSVC supplies the SDK and host tools for both architectures.
        profile["//command_line_option:host_platform"] = "//:local_windows_msvc"
        profile["//command_line_option:extra_toolchains"] += [
            "@local_config_cc//:cc-toolchain-x64_windows",
            "@local_config_cc//:cc-toolchain-arm64_windows",
        ]
    return profile

_release_profile = transition(
    implementation = _release_profile_impl,
    inputs = _PROFILE_INPUTS + ["//codex-rs:runtime_package_build_mode"],
    outputs = _PROFILE_OUTPUTS,
)

def _runtime_binary_files_impl(ctx):
    binary = ctx.attr.binary[0]
    executable = binary[DefaultInfo].files_to_run.executable
    outputs = [executable]
    validations = []
    files = [PackageFilesInfo(
        attributes = {"mode": "0755"},
        dest_src_map = {executable.basename: executable},
    )]

    # Use the binary's target configuration, including cross builds. Linux keeps
    # DWARF inside the executable until the existing packaging strip pass.
    for group, suffix in [("pdb_file", ".pdb"), ("dsym_folder", ".dSYM")]:
        symbols = getattr(binary[OutputGroupInfo], group, depset()).to_list()
        if not symbols and group == ctx.attr.symbol_group:
            fail("Missing %s for %s" % (group, binary.label))
        if not symbols:
            continue
        if len(symbols) != 1:
            fail("Expected one %s for %s, got %s" % (group, binary.label, symbols))
        if group == "dsym_folder":
            validation = ctx.actions.declare_file(ctx.label.name + ".dsym-checked")
            ctx.actions.run_shell(
                inputs = symbols,
                outputs = [validation],
                arguments = [symbols[0].path, executable.basename, validation.path],
                command = '''
                    test -s "$1/Contents/Resources/DWARF/$2" || {
                        echo "Missing or empty DWARF for $2 in $1" >&2
                        exit 1
                    }
                    touch "$3"
                ''',
                mnemonic = "ValidateRuntimeSymbols",
            )
            validations.append(validation)
        outputs.extend(symbols)
        files.append(PackageFilesInfo(
            attributes = {"mode": "0644"},
            dest_src_map = {binary.label.name + suffix: symbols[0]},
        ))

    return [
        DefaultInfo(files = depset(outputs)),
        OutputGroupInfo(_validation = depset(validations)),
        PackageFilegroupInfo(
            pkg_files = [(info, ctx.label) for info in files],
            pkg_dirs = [],
            pkg_symlinks = [],
        ),
    ]

_runtime_binary_files = rule(
    implementation = _runtime_binary_files_impl,
    attrs = {
        "binary": attr.label(mandatory = True, executable = True, cfg = _release_profile),
        "platform_os": attr.string(),
        "macos_minimum_os": attr.string(),
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
        "symbol_group": attr.string(),
    },
)

def runtime_archive(name, srcs, **kwargs):
    """Produce a flat archive of writable files, without stripping or signing."""
    pkg_tar(
        name = name,
        srcs = srcs,
        extension = "tar.gz",
        mode = "0755",
        stamp = 0,
        allow_duplicates_with_different_content = False,
        **kwargs
    )

def release_binary(binary, name = "release", **kwargs):
    """Expose release_files and release.tar.gz; override name for sibling binaries."""
    kwargs.setdefault("tags", ["manual"])
    kwargs.setdefault("visibility", ["//visibility:public"])
    _runtime_binary_files(
        name = name + "_files",
        binary = binary,
        platform_os = select({
            "@platforms//os:macos": "macos",
            "@platforms//os:linux": "linux",
            "@platforms//os:windows": "windows",
        }),
        macos_minimum_os = select({
            "@platforms//cpu:aarch64": "11.0",
            "//conditions:default": "10.12",
        }),
        symbol_group = select({
            "@platforms//os:macos": "dsym_folder",
            "@platforms//os:windows": "pdb_file",
            "//conditions:default": "",
        }),
        **kwargs
    )
    runtime_archive(name = name, srcs = [":" + name + "_files"], **kwargs)
