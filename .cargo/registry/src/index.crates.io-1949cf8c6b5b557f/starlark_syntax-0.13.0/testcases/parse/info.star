# @generated
# Copyright 2014 The Bazel Authors. All rights reserved.
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

load("@io_bazel_rules_go//go/private:mode.bzl",
    "get_mode",
)
load("@io_bazel_rules_go//go/private:actions/action.bzl",
    "add_go_env",
)
load("@io_bazel_rules_go//go/private:common.bzl",
    "declare_file",
)

def _go_info_script_impl(ctx):
  go_toolchain = ctx.toolchains["@io_bazel_rules_go//go:toolchain"]
  mode = get_mode(ctx, ctx.attr._go_toolchain_flags)
  stdlib = go_toolchain.stdlib.get(ctx, go_toolchain, mode)
  out = declare_file(ctx, ext=".bash")
  args = ctx.actions.args()
  add_go_env(args, stdlib, mode)
  args.add(["-script", "-out", out])
  ctx.actions.run(
      inputs = [],
      outputs = [out],
      mnemonic = "GoInfo",
      executable = ctx.executable._go_info,
      arguments = [args],
  )
  return [
      DefaultInfo(
          files = depset([out]),
      ),
  ]

_go_info_script = rule(
    _go_info_script_impl,
    attrs = {
        "_go_info": attr.label(
            allow_files = True,
            single_file = True,
            executable = True,
            cfg = "host",
            default="@io_bazel_rules_go//go/tools/builders:info"),
        "_go_toolchain_flags": attr.label(default=Label("@io_bazel_rules_go//go/private:go_toolchain_flags")),
    },
    toolchains = ["@io_bazel_rules_go//go:toolchain"],
)

def go_info():
  _go_info_script(
      name = "go_info_script",
      tags = ["manual"],
  )
  native.sh_binary(
      name = "go_info",
      srcs = ["go_info_script"],
      tags = ["manual"],
  )