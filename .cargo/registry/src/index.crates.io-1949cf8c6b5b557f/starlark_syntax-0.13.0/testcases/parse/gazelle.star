# @generated
# Copyright 2016 The Bazel Go Rules Authors. All rights reserved.
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

load("@io_bazel_rules_go//go/private:common.bzl",
    "declare_file",
)

_script_content = """
BASE=$(pwd)
WORKSPACE=$(dirname $(readlink WORKSPACE))
cd "$WORKSPACE"
$BASE/{gazelle} {args} $@
"""

def _gazelle_script_impl(ctx):
  prefix = ctx.attr.prefix if ctx.attr.prefix else ctx.attr._go_prefix.go_prefix
  args = [ctx.attr.command]
  args += [
      "-repo_root", "$WORKSPACE",
      "-go_prefix", prefix,
      "-external", ctx.attr.external,
      "-mode", ctx.attr.mode,
  ]
  if ctx.attr.build_tags:
    args += ["-build_tags", ",".join(ctx.attr.build_tags)]
  args += ctx.attr.args
  script_content = _script_content.format(gazelle=ctx.file._gazelle.short_path, args=" ".join(args))
  script_file = declare_file(ctx, ext=".bash")
  ctx.actions.write(output=script_file, is_executable=True, content=script_content)
  return struct(
    files = depset([script_file]),
    runfiles = ctx.runfiles([ctx.file._gazelle])
  )

def _go_prefix_default(prefix):
  return (None
          if prefix
          else Label("//:go_prefix", relative_to_caller_repository = True))

_gazelle_script = rule(
    _gazelle_script_impl,
    attrs = {
        "command": attr.string(values=["update", "fix"], default="update"),
        "mode": attr.string(values=["print", "fix", "diff"], default="fix"),
        "external": attr.string(values=["external", "vendored"], default="external"),
        "build_tags": attr.string_list(),
        "args": attr.string_list(),
        "prefix": attr.string(),
        "_gazelle": attr.label(
            default = Label("@io_bazel_rules_go//go/tools/gazelle/gazelle:gazelle"),
            allow_files = True,
            single_file = True,
            executable = True,
            cfg = "host"
        ),
        "_go_prefix": attr.label(default = _go_prefix_default),
    }
)

def gazelle(name, **kwargs):
  """See go/extras.rst#gazelle for full documentation."""
  script_name = name+"_script"
  _gazelle_script(
      name = script_name,
      tags = ["manual"],
      **kwargs
  )
  native.sh_binary(
      name = name,
      srcs = [script_name],
      data = ["//:WORKSPACE"],
      tags = ["manual"],
  )
