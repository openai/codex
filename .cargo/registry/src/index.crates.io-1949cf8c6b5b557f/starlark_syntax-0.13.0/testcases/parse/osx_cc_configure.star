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
"""Configuring the C++ toolchain on macOS."""

load("@bazel_tools//tools/osx:xcode_configure.bzl", "run_xcode_locator")

load(
    "@bazel_tools//tools/cpp:lib_cc_configure.bzl",
    "escape_string",
)

load(
    "@bazel_tools//tools/cpp:unix_cc_configure.bzl",
    "get_escaped_cxx_inc_directories",
    "tpl",
    "get_env",
    "find_cc",
    "configure_unix_toolchain"
)


def _get_escaped_xcode_cxx_inc_directories(repository_ctx, cc, xcode_toolchains):
  """Compute the list of default C++ include paths on Xcode-enabled darwin.

  Args:
    repository_ctx: The repository context.
    cc: The default C++ compiler on the local system.
    xcode_toolchains: A list containing the xcode toolchains available
  Returns:
    include_paths: A list of builtin include paths.
  """

  # TODO(cparsons): Falling back to the default C++ compiler builtin include
  # paths shouldn't be unnecessary once all actions are using xcrun.
  include_dirs = get_escaped_cxx_inc_directories(repository_ctx, cc)
  for toolchain in xcode_toolchains:
    include_dirs.append(escape_string(toolchain.developer_dir))
  return include_dirs


def configure_osx_toolchain(repository_ctx):
  """Configure C++ toolchain on macOS."""
  xcode_toolchains = []
  (xcode_toolchains, xcodeloc_err) = run_xcode_locator(
      repository_ctx,
      Label("@bazel_tools//tools/osx:xcode_locator.m"))
  if xcode_toolchains:
    cc = find_cc(repository_ctx)
    tpl(repository_ctx, "osx_cc_wrapper.sh", {
        "%{cc}": escape_string(str(cc)),
        "%{env}": escape_string(get_env(repository_ctx))
    }, "cc_wrapper.sh")
    repository_ctx.symlink(
        Label("@bazel_tools//tools/objc:xcrunwrapper.sh"), "xcrunwrapper.sh")
    repository_ctx.symlink(
        Label("@bazel_tools//tools/objc:libtool.sh"), "libtool")
    repository_ctx.symlink(
        Label("@bazel_tools//tools/objc:make_hashed_objlist.py"),
        "make_hashed_objlist.py")
    repository_ctx.symlink(
        Label("@bazel_tools//tools/osx/crosstool:wrapped_ar.tpl"),
        "wrapped_ar")
    repository_ctx.symlink(
        Label("@bazel_tools//tools/osx/crosstool:wrapped_clang.tpl"),
        "wrapped_clang")
    repository_ctx.symlink(
        Label("@bazel_tools//tools/osx/crosstool:wrapped_clang_pp.tpl"),
        "wrapped_clang_pp")
    repository_ctx.symlink(
        Label("@bazel_tools//tools/osx/crosstool:BUILD.tpl"),
        "BUILD")
    repository_ctx.symlink(
        Label("@bazel_tools//tools/osx/crosstool:osx_archs.bzl"),
        "osx_archs.bzl")
    escaped_include_paths = _get_escaped_xcode_cxx_inc_directories(repository_ctx, cc, xcode_toolchains)
    escaped_cxx_include_directories = []
    for path in escaped_include_paths:
      escaped_cxx_include_directories.append(("cxx_builtin_include_directory: \"%s\"" % path))
    if xcodeloc_err:
      escaped_cxx_include_directories.append("# Error: " + xcodeloc_err + "\n")
    repository_ctx.template(
        "CROSSTOOL",
        Label("@bazel_tools//tools/osx/crosstool:CROSSTOOL.tpl"),
        {"%{cxx_builtin_include_directory}": "\n".join(escaped_cxx_include_directories)})
  else:
    configure_unix_toolchain(repository_ctx, cpu_value = "darwin")
