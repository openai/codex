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
"""Configuring the C++ toolchain on Unix platforms."""


load(
    "@bazel_tools//tools/cpp:lib_cc_configure.bzl",
    "escape_string",
    "get_env_var",
    "which",
    "tpl",
)


def _get_value(it):
  """Convert `it` in serialized protobuf format."""
  if type(it) == "int":
    return str(it)
  elif type(it) == "bool":
    return "true" if it else "false"
  else:
    return "\"%s\"" % it


def _build_crosstool(d, prefix="  "):
  """Convert `d` to a string version of a CROSSTOOL file content."""
  lines = []
  for k in d:
    if type(d[k]) == "list":
      for it in d[k]:
        lines.append("%s%s: %s" % (prefix, k, _get_value(it)))
    else:
      lines.append("%s%s: %s" % (prefix, k, _get_value(d[k])))
  return "\n".join(lines)


def _build_tool_path(d):
  """Build the list of %-escaped tool_path for the CROSSTOOL file."""
  lines = []
  for k in d:
    lines.append("  tool_path {name: \"%s\" path: \"%s\" }" % (k, escape_string(d[k])))
  return "\n".join(lines)


def _get_tool_paths(repository_ctx, darwin, cc):
  """Compute the path to the various tools. Doesn't %-escape the result!"""
  return {k: which(repository_ctx, k, "/usr/bin/" + k)
          for k in [
              "ld",
              "cpp",
              "dwp",
              "gcov",
              "nm",
              "objcopy",
              "objdump",
              "strip",
          ]} + {
              "gcc": cc,
              "ar": "/usr/bin/libtool"
                    if darwin else which(repository_ctx, "ar", "/usr/bin/ar")
          }


def _escaped_cplus_include_paths(repository_ctx):
  """Use ${CPLUS_INCLUDE_PATH} to compute the %-escaped list of flags for cxxflag."""
  if "CPLUS_INCLUDE_PATH" in repository_ctx.os.environ:
    result = []
    for p in repository_ctx.os.environ["CPLUS_INCLUDE_PATH"].split(":"):
      p = escape_string(str(repository_ctx.path(p)))  # Normalize the path
      result.append("-I" + p)
    return result
  else:
    return []


_INC_DIR_MARKER_BEGIN = "#include <...>"

# OSX add " (framework directory)" at the end of line, strip it.
_OSX_FRAMEWORK_SUFFIX = " (framework directory)"
_OSX_FRAMEWORK_SUFFIX_LEN =  len(_OSX_FRAMEWORK_SUFFIX)

def _cxx_inc_convert(path):
  """Convert path returned by cc -E xc++ in a complete path. Doesn't %-escape the path!"""
  path = path.strip()
  if path.endswith(_OSX_FRAMEWORK_SUFFIX):
    path = path[:-_OSX_FRAMEWORK_SUFFIX_LEN].strip()
  return path


def get_escaped_cxx_inc_directories(repository_ctx, cc):
  """Compute the list of default %-escaped C++ include directories."""
  result = repository_ctx.execute([cc, "-E", "-xc++", "-", "-v"])
  index1 = result.stderr.find(_INC_DIR_MARKER_BEGIN)
  if index1 == -1:
    return []
  index1 = result.stderr.find("\n", index1)
  if index1 == -1:
    return []
  index2 = result.stderr.rfind("\n ")
  if index2 == -1 or index2 < index1:
    return []
  index2 = result.stderr.find("\n", index2 + 1)
  if index2 == -1:
    inc_dirs = result.stderr[index1 + 1:]
  else:
    inc_dirs = result.stderr[index1 + 1:index2].strip()

  return [escape_string(repository_ctx.path(_cxx_inc_convert(p)))
          for p in inc_dirs.split("\n")]


def _add_option_if_supported(repository_ctx, cc, option):
  """Checks that `option` is supported by the C compiler. Doesn't %-escape the option."""
  result = repository_ctx.execute([
      cc,
      option,
      "-o",
      "/dev/null",
      "-c",
      str(repository_ctx.path("tools/cpp/empty.cc"))
  ])
  return [option] if result.stderr.find(option) == -1 else []


def _is_gold_supported(repository_ctx, cc):
  """Checks that `gold` is supported by the C compiler."""
  result = repository_ctx.execute([
      cc,
      "-fuse-ld=gold",
      "-o",
      "/dev/null",
      # Some macos clang versions don't fail when setting -fuse-ld=gold, adding
      # these lines to force it to. This also means that we will not detect
      # gold when only a very old (year 2010 and older) is present.
      "-Wl,--start-lib",
      "-Wl,--end-lib",
      str(repository_ctx.path("tools/cpp/empty.cc"))
  ])
  return result.return_code == 0


def _crosstool_content(repository_ctx, cc, cpu_value, darwin):
  """Return the content for the CROSSTOOL file, in a dictionary."""
  supports_gold_linker = _is_gold_supported(repository_ctx, cc)
  return {
      "abi_version": escape_string(get_env_var(repository_ctx, "ABI_VERSION", "local", False)),
      "abi_libc_version": escape_string(get_env_var(repository_ctx, "ABI_LIBC_VERSION", "local", False)),
      "builtin_sysroot": "",
      "compiler": escape_string(get_env_var(repository_ctx, "BAZEL_COMPILER", "compiler", False)),
      "host_system_name": escape_string(get_env_var(repository_ctx, "BAZEL_HOST_SYSTEM", "local", False)),
      "needsPic": True,
      "supports_gold_linker": supports_gold_linker,
      "supports_incremental_linker": False,
      "supports_fission": False,
      "supports_interface_shared_objects": False,
      "supports_normalizing_ar": False,
      "supports_start_end_lib": supports_gold_linker,
      "target_libc": "macosx" if darwin else escape_string(get_env_var(repository_ctx, "BAZEL_TARGET_LIBC", "local", False)),
      "target_cpu": escape_string(get_env_var(repository_ctx, "BAZEL_TARGET_CPU", cpu_value, False)),
      "target_system_name": escape_string(get_env_var(repository_ctx, "BAZEL_TARGET_SYSTEM", "local", False)),
      "cxx_flag": [
          "-std=c++0x",
      ] + _escaped_cplus_include_paths(repository_ctx),
      "linker_flag": [
          "-lstdc++",
          "-lm",  # Some systems expect -lm in addition to -lstdc++
          # Anticipated future default.
      ] + (
          ["-fuse-ld=gold"] if supports_gold_linker else []
      ) + _add_option_if_supported(
          repository_ctx, cc, "-Wl,-no-as-needed"
      ) + _add_option_if_supported(
          repository_ctx, cc, "-Wl,-z,relro,-z,now"
      ) + ([
          "-undefined",
          "dynamic_lookup",
          "-headerpad_max_install_names",
          ] if darwin else [
              "-B" + str(repository_ctx.path(cc).dirname),
              # Always have -B/usr/bin, see https://github.com/bazelbuild/bazel/issues/760.
              "-B/usr/bin",
              # Gold linker only? Can we enable this by default?
              # "-Wl,--warn-execstack",
              # "-Wl,--detect-odr-violations"
          ] + _add_option_if_supported(
              # Have gcc return the exit code from ld.
              repository_ctx, cc, "-pass-exit-codes")
          ),
      "cxx_builtin_include_directory": get_escaped_cxx_inc_directories(repository_ctx, cc),
      "objcopy_embed_flag": ["-I", "binary"],
      "unfiltered_cxx_flag":
          # If the compiler sometimes rewrites paths in the .d files without symlinks
          # (ie when they're shorter), it confuses Bazel's logic for verifying all
          # #included header files are listed as inputs to the action.
          _add_option_if_supported(repository_ctx, cc, "-fno-canonical-system-headers") + [
              # Make C++ compilation deterministic. Use linkstamping instead of these
              # compiler symbols.
              "-Wno-builtin-macro-redefined",
              "-D__DATE__=\\\"redacted\\\"",
              "-D__TIMESTAMP__=\\\"redacted\\\"",
              "-D__TIME__=\\\"redacted\\\""
          ],
      "compiler_flag": [
          # Security hardening requires optimization.
          # We need to undef it as some distributions now have it enabled by default.
          "-U_FORTIFY_SOURCE",
          "-fstack-protector",
          # All warnings are enabled. Maybe enable -Werror as well?
          "-Wall",
          # Enable a few more warnings that aren't part of -Wall.
      ] + (["-Wthread-safety", "-Wself-assign"] if darwin else [
          "-B" + escape_string(str(repository_ctx.path(cc).dirname)),
          # Always have -B/usr/bin, see https://github.com/bazelbuild/bazel/issues/760.
          "-B/usr/bin",
      ]) + (
          # Disable problematic warnings.
          _add_option_if_supported(repository_ctx, cc, "-Wunused-but-set-parameter") +
          # has false positives
          _add_option_if_supported(repository_ctx, cc, "-Wno-free-nonheap-object") +
          # Enable coloring even if there's no attached terminal. Bazel removes the
          # escape sequences if --nocolor is specified.
          _add_option_if_supported(repository_ctx, cc, "-fcolor-diagnostics")) + [
              # Keep stack frames for debugging, even in opt mode.
              "-fno-omit-frame-pointer",
          ],
  }


def _opt_content(darwin):
  """Return the content of the opt specific section of the CROSSTOOL file."""
  return {
      "compiler_flag": [
          # No debug symbols.
          # Maybe we should enable https://gcc.gnu.org/wiki/DebugFission for opt or
          # even generally? However, that can't happen here, as it requires special
          # handling in Bazel.
          "-g0",

          # Conservative choice for -O
          # -O3 can increase binary size and even slow down the resulting binaries.
          # Profile first and / or use FDO if you need better performance than this.
          "-O2",

          # Security hardening on by default.
          # Conservative choice; -D_FORTIFY_SOURCE=2 may be unsafe in some cases.
          "-D_FORTIFY_SOURCE=1",

          # Disable assertions
          "-DNDEBUG",

          # Removal of unused code and data at link time (can this increase binary size in some cases?).
          "-ffunction-sections",
          "-fdata-sections"
      ],
      "linker_flag": [] if darwin else ["-Wl,--gc-sections"]
  }


def _dbg_content():
  """Return the content of the dbg specific section of the CROSSTOOL file."""
  # Enable debug symbols
  return {"compiler_flag": "-g"}


def get_env(repository_ctx):
  """Convert the environment in a list of export if in Homebrew. Doesn't %-escape the result!"""
  env = repository_ctx.os.environ
  if "HOMEBREW_RUBY_PATH" in env:
    return "\n".join([
        "export %s='%s'" % (k, env[k].replace("'", "'\\''"))
        for k in env
        if k != "_" and k.find(".") == -1
    ])
  else:
    return ""


def _coverage_feature(darwin):
  if darwin:
    compile_flags = """flag_group {
        flag: '-fprofile-instr-generate'
        flag: '-fcoverage-mapping'
      }"""
    link_flags = """flag_group {
        flag: '-fprofile-instr-generate'
      }"""
  else:
    compile_flags = """flag_group {
        flag: '-fprofile-arcs'
        flag: '-ftest-coverage'
      }"""
    link_flags = """flag_group {
        flag: '-lgcov'
      }"""
  return """
    feature {
      name: 'coverage'
      provides: 'profile'
      flag_set {
        action: 'preprocess-assemble'
        action: 'c-compile'
        action: 'c++-compile'
        action: 'c++-header-parsing'
        action: 'c++-header-preprocessing'
        action: 'c++-module-compile'
        """ + compile_flags + """



      }
      flag_set {
        action: 'c++-link-interface-dynamic-library'
        action: 'c++-link-dynamic-library'
        action: 'c++-link-executable'
        """ + link_flags + """
      }
    }
  """


def find_cc(repository_ctx):
  """Find the C++ compiler. Doesn't %-escape the result."""
  cc_name = "gcc"
  cc_environ = repository_ctx.os.environ.get("CC")
  cc_paren = ""
  if cc_environ != None:
    cc_environ = cc_environ.strip()
    if cc_environ:
      cc_name = cc_environ
      cc_paren = " (%s)" % cc_environ
  if cc_name.startswith("/"):
    # Absolute path, maybe we should make this supported by our which function.
    return cc_name
  cc = repository_ctx.which(cc_name)
  if cc == None:
    fail(
        ("Cannot find gcc or CC%s, either correct your path or set the CC"
         + " environment variable") % cc_paren)
  return cc


def configure_unix_toolchain(repository_ctx, cpu_value):
  """Configure C++ toolchain on Unix platforms."""
  repository_ctx.file("tools/cpp/empty.cc", "int main() {}")
  darwin = cpu_value == "darwin"
  cc = find_cc(repository_ctx)
  tool_paths = _get_tool_paths(repository_ctx, darwin,
                               "cc_wrapper.sh" if darwin else str(cc))
  crosstool_content = _crosstool_content(repository_ctx, cc, cpu_value, darwin)
  opt_content = _opt_content(darwin)
  dbg_content = _dbg_content()
  tpl(repository_ctx, "BUILD", {
      "%{name}": cpu_value,
      "%{supports_param_files}": "0" if darwin else "1",
      "%{cc_compiler_deps}": ":cc_wrapper" if darwin else ":empty",
      "%{compiler}": get_env_var(repository_ctx, "BAZEL_COMPILER", "compiler", False),
  })
  tpl(repository_ctx,
      "osx_cc_wrapper.sh" if darwin else "linux_cc_wrapper.sh",
      {"%{cc}": escape_string(str(cc)),
       "%{env}": escape_string(get_env(repository_ctx))},
      "cc_wrapper.sh")
  tpl(repository_ctx, "CROSSTOOL", {
      "%{cpu}": escape_string(cpu_value),
      "%{default_toolchain_name}": escape_string(
          get_env_var(repository_ctx,
                      "CC_TOOLCHAIN_NAME",
                      "local",
                      False)),
      "%{toolchain_name}": escape_string(
          get_env_var(repository_ctx, "CC_TOOLCHAIN_NAME", "local", False)),
      "%{content}": _build_crosstool(crosstool_content) + "\n" +
                    _build_tool_path(tool_paths),
      "%{opt_content}": _build_crosstool(opt_content, "    "),
      "%{dbg_content}": _build_crosstool(dbg_content, "    "),
      "%{cxx_builtin_include_directory}": "",
      "%{coverage}": _coverage_feature(darwin),
      "%{msvc_env_tmp}": "",
      "%{msvc_env_path}": "",
      "%{msvc_env_include}": "",
      "%{msvc_env_lib}": "",
      "%{msvc_cl_path}": "",
      "%{msvc_ml_path}": "",
      "%{msvc_link_path}": "",
      "%{msvc_lib_path}": "",
      "%{dbg_mode_debug}": "",
      "%{fastbuild_mode_debug}": "",
      "%{compilation_mode_content}": "",
  })
