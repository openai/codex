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
load("@io_bazel_rules_go//go/private:common.bzl",
    "to_set",
    "sets",
)

def emit_asm(ctx, go_toolchain,
    source = None,
    hdrs = [],
    out_obj = None,
    mode = None):
  """See go/toolchains.rst#asm for full documentation."""

  if source == None: fail("source is a required parameter")
  if out_obj == None: fail("out_obj is a required parameter")
  if mode == None: fail("mode is a required parameter")

  stdlib = go_toolchain.stdlib.get(ctx, go_toolchain, mode)
  includes = to_set([stdlib.root_file.dirname + "/pkg/include"])
  includes = sets.union(includes, [f.dirname for f in hdrs])
  inputs = hdrs + stdlib.files + [source]

  asm_args = ctx.actions.args()
  add_go_env(asm_args, stdlib, mode)
  asm_args.add(["-o", out_obj, "-trimpath", "."])
  asm_args.add(includes, before_each="-I")
  asm_args.add(source.path)
  ctx.actions.run(
      inputs = inputs,
      outputs = [out_obj],
      mnemonic = "GoAsmCompile",
      executable = go_toolchain.tools.asm,
      arguments = [asm_args],
  )
