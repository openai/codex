# @generated
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

load("@io_bazel_rules_go//go/private:actions/action.bzl",
    "add_go_env",
)
load("@io_bazel_rules_go//go/private:providers.bzl",
    "GoSource",
    "GoSourceList",
)
load("@io_bazel_rules_go//go/private:common.bzl",
    "declare_file",
    "structs",
)

def emit_cover(ctx, go_toolchain,
               source = None,
               mode = None,
               importpath = ""):
  """See go/toolchains.rst#cover for full documentation."""

  if source == None: fail("source is a required parameter")
  if mode == None: fail("mode is a required parameter")
  if not importpath: fail("importpath is a required parameter")

  stdlib = go_toolchain.stdlib.get(ctx, go_toolchain, mode)

  covered = []
  cover_vars = []
  for s in source.entries:
    if not s.want_coverage:
      covered.append(s)
      continue
    outputs = []
    for src in s.srcs:
      if not src.basename.endswith(".go"):
        outputs.append(src)
        continue
      cover_var = "Cover_" + src.basename[:-3].replace("-", "_").replace(".", "_")
      cover_vars.append("{}={}={}".format(cover_var, src.short_path, importpath))
      out = declare_file(ctx, path=cover_var, ext='.cover.go')
      outputs.append(out)
      args = ctx.actions.args()
      add_go_env(args, stdlib, mode)
      args.add(["--", "--mode=set", "-var=%s" % cover_var, "-o", out, src])
      ctx.actions.run(
          inputs = [src] + stdlib.files,
          outputs = [out],
          mnemonic = "GoCover",
          executable = go_toolchain.tools.cover,
          arguments = [args],
      )

    members = structs.to_dict(s)
    members["srcs"] = outputs
    covered.append(GoSource(**members))
  return GoSourceList(entries=covered), cover_vars
