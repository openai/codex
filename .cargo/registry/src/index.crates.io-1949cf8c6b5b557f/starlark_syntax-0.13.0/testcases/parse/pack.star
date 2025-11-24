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

def emit_pack(ctx, go_toolchain,
    in_lib = None,
    out_lib = None,
    objects = [],
    archive = None,
    mode = None):
  """See go/toolchains.rst#pack for full documentation."""

  if in_lib == None: fail("in_lib is a required parameter")
  if out_lib == None: fail("out_lib is a required parameter")
  if mode == None: fail("mode is a required parameter")

  stdlib = go_toolchain.stdlib.get(ctx, go_toolchain, mode)
  inputs = [in_lib] + stdlib.files

  arguments = ctx.actions.args()
  add_go_env(arguments, stdlib, mode)
  arguments.add([
      "-in", in_lib,
      "-out", out_lib,
  ])
  inputs.extend(objects)
  arguments.add(objects, before_each="-obj")

  if archive:
    inputs.append(archive)
    arguments.add(["-arc", archive])

  ctx.actions.run(
      inputs = inputs,
      outputs = [out_lib],
      mnemonic = "GoPack",
      executable = go_toolchain.tools.pack,
      arguments = [arguments],
  )
