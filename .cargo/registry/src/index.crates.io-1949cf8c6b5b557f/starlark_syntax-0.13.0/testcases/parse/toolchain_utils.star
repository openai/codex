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

"""Finds the c++ toolchain if it is enabled.

Otherwise, falls back to a toolchain constructed from the CppConfiguration.
"""

def _get_cpp_toolchain_attr(ctx, attr):
  if hasattr(ctx.attr._cc_toolchain[cc_common.CcToolchainInfo], attr):
    return getattr(ctx.attr._cc_toolchain[cc_common.CcToolchainInfo], attr)
  else:
    return getattr(ctx.fragments.cpp, attr)

def _make_legacy_toolchain(ctx):
    return struct(
        objcopy_executable = _get_cpp_toolchain_attr(ctx, "objcopy_executable"),
        compiler_executable = _get_cpp_toolchain_attr(ctx, "compiler_executable"),
        preprocessor_executable = _get_cpp_toolchain_attr(ctx, "preprocessor_executable"),
        nm_executable = _get_cpp_toolchain_attr(ctx, "nm_executable"),
        objdump_executable = _get_cpp_toolchain_attr(ctx, "objdump_executable"),
        ar_executable = _get_cpp_toolchain_attr(ctx, "ar_executable"),
        strip_executable = _get_cpp_toolchain_attr(ctx, "strip_executable"),
        ld_executable = _get_cpp_toolchain_attr(ctx, "ld_executable"),
    )

def find_cpp_toolchain(ctx):
  """If the c++ toolchain is in use, returns it.

  Otherwise, returns a c++ toolchain derived from legacy toolchain selection.

  Args:
    ctx: The rule context for which to find a toolchain.

  Returns:
    A CcToolchainProvider.
  """

  if Label("@bazel_tools//tools/cpp:toolchain_type") in ctx.fragments.platform.enabled_toolchain_types:
    return ctx.toolchains["@bazel_tools//tools/cpp:toolchain_type"]
  else:
    return _make_legacy_toolchain(ctx)
