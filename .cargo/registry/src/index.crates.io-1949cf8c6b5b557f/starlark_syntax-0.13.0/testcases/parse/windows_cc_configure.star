# @generated
# pylint: disable=g-bad-file-header
# Copyright 2016 The Bazel Authors. All rights reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#    http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
"""Configuring the C++ toolchain on Windows."""

load(
    "@bazel_tools//tools/cpp:lib_cc_configure.bzl",
    "escape_string",
    "auto_configure_fail",
    "auto_configure_warning",
    "get_env_var",
    "which",
    "which_cmd",
    "execute",
    "tpl",
    "is_cc_configure_debug",
)


# TODO(pcloudy): Remove this after MSVC CROSSTOOL becomes default on Windows
def _get_escaped_windows_msys_crosstool_content(repository_ctx):
  """Return the content of msys crosstool which is still the default CROSSTOOL on Windows."""
  bazel_sh = get_env_var(repository_ctx, "BAZEL_SH").replace("\\", "/").lower()
  tokens = bazel_sh.rsplit("/", 1)
  msys_root = None
  if tokens[0].endswith("/usr/bin"):
    msys_root = tokens[0][:len(tokens[0]) - len("usr/bin")]
  elif tokens[0].endswith("/bin"):
    msys_root = tokens[0][:len(tokens[0]) - len("bin")]
  if not msys_root:
    auto_configure_fail(
        "Could not determine MSYS/Cygwin root from BAZEL_SH (%s)" % bazel_sh)
  escaped_msys_root = escape_string(msys_root)
  return (
      '   abi_version: "local"\n' +
      '   abi_libc_version: "local"\n' +
      '   builtin_sysroot: ""\n' +
      '   compiler: "windows_msys64"\n' +
      '   host_system_name: "local"\n' +
      "   needsPic: false\n" +
      '   target_libc: "local"\n' +
      '   target_cpu: "x64_windows_msys"\n' +
      '   target_system_name: "local"\n' +
      '   tool_path { name: "ar" path: "%susr/bin/ar" }\n' % escaped_msys_root +
      '   tool_path { name: "compat-ld" path: "%susr/bin/ld" }\n' % escaped_msys_root +
      '   tool_path { name: "cpp" path: "%susr/bin/cpp" }\n' % escaped_msys_root +
      '   tool_path { name: "dwp" path: "%susr/bin/dwp" }\n' % escaped_msys_root +
      '   tool_path { name: "gcc" path: "%susr/bin/gcc" }\n' % escaped_msys_root +
      '   cxx_flag: "-std=gnu++0x"\n' +
      '   linker_flag: "-lstdc++"\n' +
      '   cxx_builtin_include_directory: "%s"\n' % escaped_msys_root +
      '   cxx_builtin_include_directory: "/usr/"\n' +
      '   tool_path { name: "gcov" path: "%susr/bin/gcov" }\n' % escaped_msys_root +
      '   tool_path { name: "ld" path: "%susr/bin/ld" }\n' % escaped_msys_root +
      '   tool_path { name: "nm" path: "%susr/bin/nm" }\n' % escaped_msys_root +
      '   tool_path { name: "objcopy" path: "%susr/bin/objcopy" }\n' % escaped_msys_root +
      '   objcopy_embed_flag: "-I"\n' +
      '   objcopy_embed_flag: "binary"\n' +
      '   tool_path { name: "objdump" path: "%susr/bin/objdump" }\n' % escaped_msys_root +
      '   tool_path { name: "strip" path: "%susr/bin/strip" }'% escaped_msys_root )


def _get_system_root(repository_ctx):
  r"""Get System root path on Windows, default is C:\\Windows. Doesn't %-escape the result."""
  if "SYSTEMROOT" in repository_ctx.os.environ:
    return escape_string(repository_ctx.os.environ["SYSTEMROOT"])
  auto_configure_warning("SYSTEMROOT is not set, using default SYSTEMROOT=C:\\Windows")
  return "C:\\Windows"


def _find_cuda(repository_ctx):
  """Find out if and where cuda is installed. Doesn't %-escape the result."""
  if "CUDA_PATH" in repository_ctx.os.environ:
    return repository_ctx.os.environ["CUDA_PATH"]
  nvcc = which(repository_ctx, "nvcc.exe")
  if nvcc:
    return nvcc[:-len("/bin/nvcc.exe")]
  return None


def _find_python(repository_ctx):
  """Find where is python on Windows. Doesn't %-escape the result."""
  if "BAZEL_PYTHON" in repository_ctx.os.environ:
    python_binary = repository_ctx.os.environ["BAZEL_PYTHON"]
    if not python_binary.endswith(".exe"):
      python_binary = python_binary + ".exe"
    return python_binary
  auto_configure_warning("'BAZEL_PYTHON' is not set, start looking for python in PATH.")
  python_binary = which_cmd(repository_ctx, "python.exe")
  auto_configure_warning("Python found at %s" % python_binary)
  return python_binary


def _add_system_root(repository_ctx, env):
  r"""Running VCVARSALL.BAT and VCVARSQUERYREGISTRY.BAT need %SYSTEMROOT%\\system32 in PATH."""
  if "PATH" not in env:
    env["PATH"] = ""
  env["PATH"] = env["PATH"] + ";" + _get_system_root(repository_ctx) + "\\system32"
  return env


def find_vc_path(repository_ctx):
  """Find Visual C++ build tools install path. Doesn't %-escape the result."""
  # 1. Check if BAZEL_VC or BAZEL_VS is already set by user.
  if "BAZEL_VC" in repository_ctx.os.environ:
    return repository_ctx.os.environ["BAZEL_VC"]

  if "BAZEL_VS" in repository_ctx.os.environ:
    return repository_ctx.os.environ["BAZEL_VS"] + "\\VC\\"
  auto_configure_warning("'BAZEL_VC' is not set, " +
                         "start looking for the latest Visual C++ installed.")

  # 2. Check if VS%VS_VERSION%COMNTOOLS is set, if true then try to find and use
  # vcvarsqueryregistry.bat to detect VC++.
  auto_configure_warning("Looking for VS%VERSION%COMNTOOLS environment variables," +
                         "eg. VS140COMNTOOLS")
  for vscommontools_env in ["VS140COMNTOOLS", "VS120COMNTOOLS",
                            "VS110COMNTOOLS", "VS100COMNTOOLS", "VS90COMNTOOLS"]:
    if vscommontools_env not in repository_ctx.os.environ:
      continue
    vcvarsqueryregistry = repository_ctx.os.environ[vscommontools_env] + "\\vcvarsqueryregistry.bat"
    if not repository_ctx.path(vcvarsqueryregistry).exists:
      continue
    repository_ctx.file("get_vc_dir.bat",
                        "@echo off\n" +
                        "call \"" + vcvarsqueryregistry + "\"\n" +
                        "echo %VCINSTALLDIR%", True)
    env = _add_system_root(repository_ctx, repository_ctx.os.environ)
    vc_dir = execute(repository_ctx, ["./get_vc_dir.bat"], environment=env)

    auto_configure_warning("Visual C++ build tools found at %s" % vc_dir)
    return vc_dir

  # 3. User might clean up all environment variables, if so looking for Visual C++ through registry.
  # Works for all VS versions, including Visual Studio 2017.
  auto_configure_warning("Looking for Visual C++ through registry")
  reg_binary = _get_system_root(repository_ctx) + "\\system32\\reg.exe"
  vc_dir = None
  for key, suffix in (("VC7", ""), ("VS7", "\\VC")):
    for version in ["15.0", "14.0", "12.0", "11.0", "10.0", "9.0", "8.0"]:
      if vc_dir:
        break
      result = repository_ctx.execute([reg_binary, "query", "HKEY_LOCAL_MACHINE\\SOFTWARE\\Wow6432Node\\Microsoft\\VisualStudio\\SxS\\" + key, "/v", version])
      if is_cc_configure_debug(repository_ctx):
        auto_configure_warning("registry query result for VC %s:\n\nSTDOUT(start)\n%s\nSTDOUT(end)\nSTDERR(start):\n%s\nSTDERR(end)\n" %
                               (version, result.stdout, result.stderr))
      if not result.stderr:
        for line in result.stdout.split("\n"):
          line = line.strip()
          if line.startswith(version) and line.find("REG_SZ") != -1:
            vc_dir = line[line.find("REG_SZ") + len("REG_SZ"):].strip() + suffix

  if not vc_dir:
    return "visual-studio-not-found"
  auto_configure_warning("Visual C++ build tools found at %s" % vc_dir)
  return vc_dir


def _is_vs_2017(vc_path):
  """Check if the installed VS version is Visual Studio 2017."""
  # In VS 2017, the location of VC is like:
  # C:\Program Files (x86)\Microsoft Visual Studio\2017\BuildTools\VC\
  # In VS 2015 or older version, it is like:
  # C:\Program Files (x86)\Microsoft Visual Studio 14.0\VC\
  return vc_path.find("2017") != -1


def _find_vcvarsall_bat_script(repository_ctx, vc_path):
  """Find vcvarsall.bat script. Doesn't %-escape the result."""
  if _is_vs_2017(vc_path):
    vcvarsall = vc_path + "\\Auxiliary\\Build\\VCVARSALL.BAT"
  else:
    vcvarsall = vc_path + "\\VCVARSALL.BAT"

  if not repository_ctx.path(vcvarsall).exists:
    auto_configure_fail(vcvarsall + " doesn't exist, please check your VC++ installation")
  return vcvarsall


def _find_env_vars(repository_ctx, vc_path):
  """Get environment variables set by VCVARSALL.BAT. Doesn't %-escape the result!"""
  vcvarsall = _find_vcvarsall_bat_script(repository_ctx, vc_path)
  repository_ctx.file("get_env.bat",
                      "@echo off\n" +
                      "call \"" + vcvarsall + "\" amd64 > NUL \n" +
                      "echo PATH=%PATH%,INCLUDE=%INCLUDE%,LIB=%LIB% \n", True)
  env = _add_system_root(repository_ctx,
                         {"PATH": "", "INCLUDE": "", "LIB": ""})
  envs = execute(repository_ctx, ["./get_env.bat"], environment=env).split(",")
  env_map = {}
  for env in envs:
    key, value = env.split("=", 1)
    env_map[key] = escape_string(value.replace("\\", "\\\\"))
  return env_map


def find_msvc_tool(repository_ctx, vc_path, tool):
  """Find the exact path of a specific build tool in MSVC. Doesn't %-escape the result."""
  tool_path = ""
  if _is_vs_2017(vc_path):
    # For VS 2017, the tools are under a directory like:
    # C:\Program Files (x86)\Microsoft Visual Studio\2017\BuildTools\VC\Tools\MSVC\14.10.24930\bin\HostX64\x64
    dirs = repository_ctx.path(vc_path + "\\Tools\\MSVC").readdir()
    if len(dirs) < 1:
      auto_configure_fail("VC++ build tools directory not found under " + vc_path + "\\Tools\\MSVC")
    # Normally there should be only one child directory under %VC_PATH%\TOOLS\MSVC,
    # but iterate every directory to be more robust.
    for path in dirs:
      tool_path = str(path) + "\\bin\\HostX64\\x64\\" + tool
      if repository_ctx.path(tool_path).exists:
        break
  else:
    # For VS 2015 and older version, the tools are under:
    # C:\Program Files (x86)\Microsoft Visual Studio 14.0\VC\bin\amd64
    tool_path = vc_path + "\\bin\\amd64\\" + tool

  if not repository_ctx.path(tool_path).exists:
    auto_configure_fail(tool_path + " not found, please check your VC++ installation.")
  return tool_path


def _is_support_whole_archive(repository_ctx, vc_path):
  """Run MSVC linker alone to see if it supports /WHOLEARCHIVE."""
  env = repository_ctx.os.environ
  if "NO_WHOLE_ARCHIVE_OPTION" in env and env["NO_WHOLE_ARCHIVE_OPTION"] == "1":
    return False
  linker = find_msvc_tool(repository_ctx, vc_path, "link.exe")
  result = execute(repository_ctx, [linker], expect_failure = True)
  return result.find("/WHOLEARCHIVE") != -1


def _is_support_debug_fastlink(repository_ctx, vc_path):
  """Run MSVC linker alone to see if it supports /DEBUG:FASTLINK."""
  linker = find_msvc_tool(repository_ctx, vc_path, "link.exe")
  result = execute(repository_ctx, [linker], expect_failure = True)
  return result.find("/DEBUG[:{FASTLINK|FULL|NONE}]") != -1


def _is_use_msvc_wrapper(repository_ctx):
  """Returns True if USE_MSVC_WRAPPER is set to 1."""
  env = repository_ctx.os.environ
  return "USE_MSVC_WRAPPER" in env and env["USE_MSVC_WRAPPER"] == "1"


def _get_compilation_mode_content():
  """Return the content for adding flags for different compilation modes when using MSVC wrapper."""
  return  "\n".join([
      "    compilation_mode_flags {",
      "      mode: DBG",
      "      compiler_flag: '-Xcompilation-mode=dbg'",
      "      linker_flag: '-Xcompilation-mode=dbg'",
      "    }",
      "    compilation_mode_flags {",
      "      mode: FASTBUILD",
      "      compiler_flag: '-Xcompilation-mode=fastbuild'",
      "      linker_flag: '-Xcompilation-mode=fastbuild'",
      "    }",
      "    compilation_mode_flags {",
      "      mode: OPT",
      "      compiler_flag: '-Xcompilation-mode=opt'",
      "      linker_flag: '-Xcompilation-mode=opt'",
      "    }"])


def _escaped_cuda_compute_capabilities(repository_ctx):
  """Returns a %-escaped list of strings representing cuda compute capabilities."""

  if "CUDA_COMPUTE_CAPABILITIES" not in repository_ctx.os.environ:
    return ["3.5", "5.2"]
  capabilities_str = escape_string(repository_ctx.os.environ["CUDA_COMPUTE_CAPABILITIES"])
  capabilities = capabilities_str.split(",")
  for capability in capabilities:
    # Workaround for Skylark's lack of support for regex. This check should
    # be equivalent to checking:
    #     if re.match("[0-9]+.[0-9]+", capability) == None:
    parts = capability.split(".")
    if len(parts) != 2 or not parts[0].isdigit() or not parts[1].isdigit():
      auto_configure_fail("Invalid compute capability: %s" % capability)
  return capabilities


def configure_windows_toolchain(repository_ctx):
  """Configure C++ toolchain on Windows."""
  repository_ctx.symlink(Label("@bazel_tools//tools/cpp:BUILD.static"), "BUILD")

  vc_path = find_vc_path(repository_ctx)
  if vc_path == "visual-studio-not-found":
    vc_path_error_script = "vc_path_not_found.bat"
    repository_ctx.symlink(Label("@bazel_tools//tools/cpp:vc_path_not_found.bat"), vc_path_error_script)
    tpl(repository_ctx, "CROSSTOOL", {
        "%{cpu}": "x64_windows",
        "%{default_toolchain_name}": "msvc_x64",
        "%{toolchain_name}": "msys_x64",
        "%{msvc_env_tmp}": "",
        "%{msvc_env_path}": "",
        "%{msvc_env_include}": "",
        "%{msvc_env_lib}": "",
        "%{msvc_cl_path}": vc_path_error_script,
        "%{msvc_link_path}": vc_path_error_script,
        "%{msvc_lib_path}": vc_path_error_script,
        "%{compilation_mode_content}": "",
        "%{content}": _get_escaped_windows_msys_crosstool_content(repository_ctx),
        "%{opt_content}": "",
        "%{dbg_content}": "",
        "%{cxx_builtin_include_directory}": "",
        "%{coverage}": "",
    })
    return

  env = _find_env_vars(repository_ctx, vc_path)
  escaped_paths = escape_string(env["PATH"])
  escaped_include_paths = escape_string(env["INCLUDE"])
  escaped_lib_paths = escape_string(env["LIB"])
  escaped_tmp_dir = escape_string(
      get_env_var(repository_ctx, "TMP", "C:\\Windows\\Temp").replace("\\", "\\\\"))
  msvc_cl_path = find_msvc_tool(repository_ctx, vc_path, "cl.exe").replace("\\", "/")
  msvc_ml_path = find_msvc_tool(repository_ctx, vc_path, "ml64.exe").replace("\\", "/")
  msvc_link_path = find_msvc_tool(repository_ctx, vc_path, "link.exe").replace("\\", "/")
  msvc_lib_path = find_msvc_tool(repository_ctx, vc_path, "lib.exe").replace("\\", "/")
  escaped_cxx_include_directories = []
  compilation_mode_content = ""

  if _is_use_msvc_wrapper(repository_ctx):
    if _is_support_whole_archive(repository_ctx, vc_path):
      support_whole_archive = "True"
    else:
      support_whole_archive = "False"
    nvcc_tmp_dir_name = escaped_tmp_dir + "\\\\nvcc_inter_files_tmp_dir"
    # Make sure nvcc.exe is in PATH
    cuda_path = _find_cuda(repository_ctx)
    if cuda_path:
      escaped_paths = escape_string(cuda_path.replace("\\", "\\\\") + "/bin;") + escaped_paths
    escaped_compute_capabilities = _escaped_cuda_compute_capabilities(repository_ctx)
    tpl(repository_ctx, "wrapper/bin/pydir/msvc_tools.py", {
        "%{lib_tool}": escape_string(msvc_lib_path),
        "%{support_whole_archive}": support_whole_archive,
        "%{cuda_compute_capabilities}": ", ".join(
            ["\"%s\"" % c for c in escaped_compute_capabilities]),
        "%{nvcc_tmp_dir_name}": nvcc_tmp_dir_name,
    })
    # nvcc will generate some source files under %{nvcc_tmp_dir_name}
    # The generated files are guaranteed to have unique name, so they can share the same tmp directory
    escaped_cxx_include_directories += [ "cxx_builtin_include_directory: \"%s\"" % nvcc_tmp_dir_name ]
    msvc_wrapper = repository_ctx.path(Label("@bazel_tools//tools/cpp:CROSSTOOL")).dirname.get_child("wrapper").get_child("bin")
    for f in ["msvc_cl.bat", "msvc_link.bat", "msvc_nop.bat"]:
      repository_ctx.symlink(msvc_wrapper.get_child(f), "wrapper/bin/" + f)
    msvc_wrapper = msvc_wrapper.get_child("pydir")
    for f in ["msvc_cl.py", "msvc_link.py"]:
      repository_ctx.symlink(msvc_wrapper.get_child(f), "wrapper/bin/pydir/" + f)
    python_binary = _find_python(repository_ctx)
    tpl(repository_ctx, "wrapper/bin/call_python.bat", {"%{python_binary}": escape_string(python_binary)})
    msvc_cl_path = "wrapper/bin/msvc_cl.bat"
    msvc_link_path = "wrapper/bin/msvc_link.bat"
    msvc_lib_path = "wrapper/bin/msvc_link.bat"
    compilation_mode_content = _get_compilation_mode_content()

  for path in escaped_include_paths.split(";"):
    if path:
      escaped_cxx_include_directories.append("cxx_builtin_include_directory: \"%s\"" % path)

  support_debug_fastlink = _is_support_debug_fastlink(repository_ctx, vc_path)

  tpl(repository_ctx, "CROSSTOOL", {
      "%{cpu}": "x64_windows",
      "%{default_toolchain_name}": "msvc_x64",
      "%{toolchain_name}": "msys_x64",
      "%{msvc_env_tmp}": escaped_tmp_dir,
      "%{msvc_env_path}": escaped_paths,
      "%{msvc_env_include}": escaped_include_paths,
      "%{msvc_env_lib}": escaped_lib_paths,
      "%{msvc_cl_path}": msvc_cl_path,
      "%{msvc_ml_path}": msvc_ml_path,
      "%{msvc_link_path}": msvc_link_path,
      "%{msvc_lib_path}": msvc_lib_path,
      "%{dbg_mode_debug}": "/DEBUG:FULL" if support_debug_fastlink else "/DEBUG",
      "%{fastbuild_mode_debug}": "/DEBUG:FASTLINK" if support_debug_fastlink else "/DEBUG",
      "%{compilation_mode_content}": compilation_mode_content,
      "%{content}": _get_escaped_windows_msys_crosstool_content(repository_ctx),
      "%{opt_content}": "",
      "%{dbg_content}": "",
      "%{cxx_builtin_include_directory}": "\n".join(escaped_cxx_include_directories),
      "%{coverage}": "",
  })
