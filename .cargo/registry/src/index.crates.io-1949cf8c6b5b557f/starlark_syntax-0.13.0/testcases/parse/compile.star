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

load("@io_bazel_rules_go//go/private:common.bzl",
    "sets",
)
load("@io_bazel_rules_go//go/private:actions/action.bzl",
    "add_go_env",
    "bootstrap_action",
)

def _importpath(l):
  return [v.data.importpath for v in l]

def _searchpath(l):
  return [v.data.searchpath for v in l]

def emit_compile(ctx, go_toolchain,
    sources = None,
    importpath = "",
    archives = [],
    mode = None,
    out_lib = None,
    gc_goopts = []):
  """See go/toolchains.rst#compile for full documentation."""

  if sources == None: fail("sources is a required parameter")
  if out_lib == None: fail("out_lib is a required parameter")
  if mode == None: fail("mode is a required parameter")

  # Add in any mode specific behaviours
  if mode.race:
    gc_goopts = gc_goopts + ["-race"]
  if mode.msan:
    gc_goopts = gc_goopts + ["-msan"]

  gc_goopts = [ctx.expand_make_variables("gc_goopts", f, {}) for f in gc_goopts]
  inputs = sets.union(sources, [go_toolchain.data.package_list])
  go_sources = [s.path for s in sources if not s.basename.startswith("_cgo")]
  cgo_sources = [s.path for s in sources if s.basename.startswith("_cgo")]

  inputs = sets.union(inputs, [archive.data.file for archive in archives])

  stdlib = go_toolchain.stdlib.get(ctx, go_toolchain, mode)
  inputs = sets.union(inputs, stdlib.files)

  args = ctx.actions.args()
  add_go_env(args, stdlib, mode)
  args.add(["-package_list", go_toolchain.data.package_list])
  args.add(go_sources, before_each="-src")
  args.add(archives, before_each="-dep", map_fn=_importpath)
  args.add(archives, before_each="-I", map_fn=_searchpath)
  args.add(["-o", out_lib, "-trimpath", ".", "-I", "."])
  args.add(["--"])
  if importpath:
    args.add(["-p", importpath])
  args.add(gc_goopts)
  args.add(go_toolchain.flags.compile)
  if mode.debug:
    args.add(["-N", "-l"])
  args.add(cgo_sources)
  ctx.actions.run(
      inputs = inputs,
      outputs = [out_lib],
      mnemonic = "GoCompile",
      executable = go_toolchain.tools.compile,
      arguments = [args],
  )

def bootstrap_compile(ctx, go_toolchain,
    sources = None,
    importpath = "",
    archives = [],
    mode = None,
    out_lib = None,
    gc_goopts = []):
  """See go/toolchains.rst#compile for full documentation."""

  if sources == None: fail("sources is a required parameter")
  if out_lib == None: fail("out_lib is a required parameter")
  if archives:  fail("compile does not accept deps in bootstrap mode")
  if mode == None: fail("mode is a required parameter")

  args = ["tool", "compile", "-o", out_lib.path]
  args.extend(gc_goopts)
  args.extend([s.path for s in sources])
  bootstrap_action(ctx, go_toolchain, mode,
      inputs = sources,
      outputs = [out_lib],
      mnemonic = "GoCompile",
      arguments = args,
  )
