"""Native Windows audio actions with complete, explicitly provisioned tool inputs."""

load("@bazel_skylib//rules/directory:providers.bzl", "DirectoryInfo")
load("@rules_python//python:py_runtime_info.bzl", "PyRuntimeInfo")

WindowsBuildToolsInfo = provider(
    doc = "Selected native Windows tools, their support closure and recipe input document.",
    fields = {
        "files": "Complete declared executable, support, header and library files.",
        "inputs": "Existing windows_build_inputs schema, with execroot-relative tool paths.",
        "manifest": "Imported installed-tree manifest File.",
        "installed_files": "Files supplied by the explicitly selected installed tool repository.",
        "python": "Declared PyRuntimeInfo used by the actions.",
        "environment": "Explicit Windows OS environment; never a developer PATH.",
    },
)

def _windows_tools_impl(ctx):
    architectures = {"x86_64-pc-windows-msvc": "ml64.exe", "aarch64-pc-windows-msvc": "armasm64.exe"}
    if ctx.attr.target not in architectures:
        fail("Windows native tools require a supported MSVC target")
    python = ctx.attr._python[PyRuntimeInfo]
    if not python.interpreter:
        fail("Windows native tools require a declared native Python interpreter")
    system_root = ctx.configuration.default_shell_env.get("SystemRoot")
    if not system_root:
        fail("Pass --action_env=SystemRoot=<actual Windows directory> as a fixed value")
    host_architecture = ctx.configuration.default_shell_env.get("PROCESSOR_ARCHITECTURE")
    if not host_architecture:
        fail("Pass --action_env=PROCESSOR_ARCHITECTURE=<actual host architecture> as a fixed value")
    msvc = ctx.attr.msvc[DirectoryInfo]
    sdk = ctx.attr.sdk[DirectoryInfo]
    tools = {
        name: msvc.get_file(filename)
        for name, filename in {
            "cc": "cl.exe",
            "cxx": "cl.exe",
            "link": "link.exe",
            "lib": "lib.exe",
            "dumpbin": "dumpbin.exe",
            "bootstrap_make": "nmake.exe",
            "assembler": architectures[ctx.attr.target],
        }.items()
    }
    tools.update({"rc": sdk.get_file("rc.exe"), "mt": sdk.get_file("mt.exe")})
    tools.update({"cmake": ctx.file.cmake, "python": python.interpreter})
    installed = ctx.files.installed_tools
    manifests = [file for file in installed if file.basename == "voice-tools.json"]
    if len(manifests) != 1:
        fail("Select a complete installed tool repository with --//third_party/voice:windows_installed_tools=<tools label>")
    pkgconf_root = manifests[0].dirname + "/pkgconf-image/"
    pkgconf_files = [file for file in installed if file.path.startswith(pkgconf_root)]
    pkgconf = [file for file in pkgconf_files if file.basename == "pkgconf.exe"]
    if len(pkgconf) != 1:
        fail("Installed tool repository must declare exactly one pkgconf-image pkgconf.exe")
    tools["pkg_config"] = pkgconf[0]
    if ctx.file.cmake not in ctx.files.cmake_data:
        fail("CMake executable must belong to its declared support tree")
    includes = [target[DirectoryInfo] for target in ctx.attr.includes]
    libraries = [target[DirectoryInfo] for target in ctx.attr.libraries]
    files = depset(
        tools.values() + installed,
        transitive = [msvc.transitive_files, sdk.transitive_files, python.files, ctx.attr.cmake_data.files] +
                     [directory.transitive_files for directory in includes + libraries],
    )
    return [DefaultInfo(
        files = depset(pkgconf),
        runfiles = ctx.runfiles(files = pkgconf_files),
    ), WindowsBuildToolsInfo(
        files = files,
        python = python,
        manifest = manifests[0],
        installed_files = installed,
        environment = {
            "SystemRoot": system_root,
            "SYSTEMROOT": system_root,
            "PROCESSOR_ARCHITECTURE": host_architecture,
        },
        inputs = {
            "schemaVersion": 1,
            "target": ctx.attr.target,
            "systemRoot": system_root,
            "tools": {name: file.path for name, file in tools.items()},
            "INCLUDE": [directory.path for directory in includes],
            "LIB": [directory.path for directory in libraries],
        },
    )]

windows_build_tools = rule(
    implementation = _windows_tools_impl,
    attrs = {
        "target": attr.string(mandatory = True),
        "msvc": attr.label(mandatory = True, providers = [DirectoryInfo]),
        "sdk": attr.label(mandatory = True, providers = [DirectoryInfo]),
        "includes": attr.label_list(mandatory = True, providers = [DirectoryInfo]),
        "libraries": attr.label_list(mandatory = True, providers = [DirectoryInfo]),
        "installed_tools": attr.label(mandatory = True),
        "cmake": attr.label(default = "@cmake-3.31.8-windows-x86_64//:cmake_bin", allow_single_file = True),
        "cmake_data": attr.label(default = "@cmake-3.31.8-windows-x86_64//:cmake_data"),
        "_python": attr.label(default = "@python_3_12//:py3_runtime", cfg = "exec"),
    },
)

def _windows_prefix_impl(ctx):
    tools = ctx.attr.build_tools[WindowsBuildToolsInfo]
    prefix = ctx.actions.declare_file(ctx.label.name + "/prefix.tar")
    receipt = ctx.actions.declare_file(ctx.label.name + "/built.json")
    config = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(config, json.encode({
        "inputs": tools.inputs,
        "manifest": tools.manifest.path,
        "installed_files": [file.path for file in tools.installed_files],
        "archives": [file.path for file in ctx.files.archives],
        "prefix": prefix.path,
        "receipt": receipt.path,
    }))
    ctx.actions.run(
        executable = tools.python.interpreter,
        arguments = [ctx.file._driver.path, "build", config.path],
        inputs = depset([config, ctx.file._driver] + ctx.files.archives + ctx.files._recipe, transitive = [tools.files]),
        outputs = [prefix, receipt],
        env = tools.environment,
        execution_requirements = {"no-remote-exec": "1"},
        mnemonic = "VoiceWindowsPrefix",
    )
    return [DefaultInfo(files = depset([prefix])), OutputGroupInfo(receipt = depset([receipt]))]

windows_native_prefix = rule(
    implementation = _windows_prefix_impl,
    attrs = {
        "build_tools": attr.label(mandatory = True, cfg = "exec", providers = [WindowsBuildToolsInfo]),
        "archives": attr.label_list(mandatory = True, allow_files = True),
        "_driver": attr.label(default = "//third_party/voice:bazel_windows.py", allow_single_file = True),
        "_recipe": attr.label(default = "//third_party/voice:native_recipe"),
    },
)
