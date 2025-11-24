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
    "to_set",
)
load("@io_bazel_rules_go//go/private:mode.bzl",
    "LINKMODE_NORMAL",
)
load("@io_bazel_rules_go//go/private:actions/action.bzl",
    "add_go_env",
    "bootstrap_action",
)

def emit_link(ctx, go_toolchain,
    archive = None,
    mode = None,
    executable = None,
    gc_linkopts = [],
    x_defs = {}):
  """See go/toolchains.rst#link for full documentation."""

  if archive == None: fail("archive is a required parameter")
  if executable == None: fail("executable is a required parameter")
  if mode == None: fail("mode is a required parameter")

  stdlib = go_toolchain.stdlib.get(ctx, go_toolchain, mode)

  config_strip = len(ctx.configuration.bin_dir.path) + 1
  pkg_depth = executable.dirname[config_strip:].count('/') + 1

  ld = None
  extldflags = []
  if stdlib.cgo_tools:
    ld = stdlib.cgo_tools.compiler_executable
    extldflags.extend(stdlib.cgo_tools.options)
  extldflags.extend(["-Wl,-rpath,$ORIGIN/" + ("../" * pkg_depth)])

  gc_linkopts, extldflags = _extract_extldflags(gc_linkopts, extldflags)

  # Add in any mode specific behaviours
  if mode.race:
    gc_linkopts.append("-race")
  if mode.msan:
    gc_linkopts.append("-msan")
  if mode.static:
    gc_linkopts.extend(["-linkmode", "external"])
    extldflags.append("-static")
  if mode.link != LINKMODE_NORMAL:
    fail("Link mode {} is not yet supported".format(mode.link))

  link_opts = ["-L", "."]

  for p in archive.searchpaths: #TODO delay this depset expansion:
    link_opts.extend(["-L", p])

  for d in archive.cgo_deps:
    if d.basename.endswith('.so'):
      short_dir = d.dirname[len(d.root.path):]
      extldflags.extend(["-Wl,-rpath,$ORIGIN/" + ("../" * pkg_depth) + short_dir])

  link_opts.extend(["-o", executable.path])
  link_opts.extend(gc_linkopts)

  # Process x_defs, either adding them directly to linker options, or
  # saving them to process through stamping support.
  stamp_x_defs = {}
  for k, v in x_defs.items():
    if v.startswith("{") and v.endswith("}"):
      stamp_x_defs[k] = v[1:-1]
    else:
      link_opts.extend(["-X", "%s=%s" % (k, v)])

  link_opts.extend(go_toolchain.flags.link)
  if mode.strip:
    link_opts.extend(["-w"])

  if ld:
    link_opts.extend([
        "-extld", ld,
        "-extldflags", " ".join(extldflags),
    ])
  link_opts.append(archive.data.file.path)
  link_args = ctx.actions.args()
  add_go_env(link_args, stdlib, mode)
  # Stamping support
  stamp_inputs = []
  if stamp_x_defs or ctx.attr.linkstamp:
    stamp_inputs = [ctx.info_file, ctx.version_file]
    link_args.add(stamp_inputs, before_each="-stamp")
    for k,v in stamp_x_defs.items():
      link_args.add(["-X", "%s=%s" % (k, v)])
    # linkstamp option support: read workspace status files,
    # converting "KEY value" lines to "-X $linkstamp.KEY=value" arguments
    # to the go linker.
    if ctx.attr.linkstamp:
      link_args.add(["-linkstamp", ctx.attr.linkstamp])

  link_args.add("--")
  link_args.add(link_opts)

  ctx.actions.run(
      inputs = sets.union(archive.libs, archive.cgo_deps,
                go_toolchain.data.crosstool, stamp_inputs, stdlib.files),
      outputs = [executable],
      mnemonic = "GoLink",
      executable = go_toolchain.tools.link,
      arguments = [link_args],
  )

def bootstrap_link(ctx, go_toolchain,
    archive = None,
    mode = None,
    executable = None,
    gc_linkopts = [],
    x_defs = {}):
  """See go/toolchains.rst#link for full documentation."""

  if archive == None: fail("archive is a required parameter")
  if executable == None: fail("executable is a required parameter")
  if mode == None: fail("mode is a required parameter")

  if x_defs:  fail("link does not accept x_defs in bootstrap mode")

  inputs = depset([archive.data.file])
  args = ["tool", "link", "-o", executable.path]
  args.extend(gc_linkopts)
  args.append(archive.data.file.path)
  bootstrap_action(ctx, go_toolchain, mode,
      inputs = inputs,
      outputs = [executable],
      mnemonic = "GoCompile",
      arguments = args,
  )

def _extract_extldflags(gc_linkopts, extldflags):
  """Extracts -extldflags from gc_linkopts and combines them into a single list.

  Args:
    gc_linkopts: a list of flags passed in through the gc_linkopts attributes.
      ctx.expand_make_variables should have already been applied.
    extldflags: a list of flags to be passed to the external linker.

  Return:
    A tuple containing the filtered gc_linkopts with external flags removed,
    and a combined list of external flags.
  """
  filtered_gc_linkopts = []
  is_extldflags = False
  for opt in gc_linkopts:
    if is_extldflags:
      is_extldflags = False
      extldflags.append(opt)
    elif opt == "-extldflags":
      is_extldflags = True
    else:
      filtered_gc_linkopts.append(opt)
  return filtered_gc_linkopts, extldflags
