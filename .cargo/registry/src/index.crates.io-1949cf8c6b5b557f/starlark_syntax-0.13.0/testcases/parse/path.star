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

load("@io_bazel_rules_go//go/private:providers.bzl", "GoLibrary", "GoPath")
load("@io_bazel_rules_go//go/private:common.bzl", "declare_file")


def _tag(ctx, path, outputs):
  """this generates a existence tag file for dependencies, and returns the path to the tag file"""
  tag = declare_file(ctx, path=path+".tag")
  path, _, _ = tag.short_path.rpartition("/")
  ctx.actions.write(tag, content="")
  outputs.append(tag)
  return path

def _go_path_impl(ctx):
  print("""
EXPERIMENTAL: the go_path rule is still very experimental
Please do not rely on it for production use, but feel free to use it and file issues
""")
  go_toolchain = ctx.toolchains["@io_bazel_rules_go//go:toolchain"]
  # First gather all the library rules
  golibs = depset()
  for dep in ctx.attr.deps:
    golibs += dep[GoLibrary].transitive

  # Now scan them for sources
  seen_libs = {}
  seen_paths = {}
  outputs = []
  packages = []
  for golib in golibs:
    if golib.importpath in seen_libs:
      # We found two different library rules that map to the same import path
      # This is legal in bazel, but we can't build a valid go path for it.
      # TODO: we might be able to ignore this if the content is identical
      print("""Duplicate package
Found {} in
  {}
  {}
""".format(golib.importpath, golib.name, seen_libs[golib.importpath].name))
      # for now we don't fail if we see duplicate packages
      # the most common case is the same source from two different workspaces
      continue
    seen_libs[golib.importpath] = golib
    package_files = []
    prefix = "src/" + golib.importpath + "/"
    for src in golib.srcs:
      outpath = prefix + src.basename
      if outpath in seen_paths:
        # If we see the same path twice, it's a fatal error
        fail("Duplicate path {}".format(outpath))
      seen_paths[outpath] = True
      out = declare_file(ctx, path=outpath)
      package_files += [out]
      outputs += [out]
      if ctx.attr.mode == "copy":
        ctx.actions.expand_template(template=src, output=out, substitutions={})
      elif ctx.attr.mode == "link":
        ctx.actions.run_shell(
            command='ln -s $(readlink "$1") "$2"',
            arguments=[src.path, out.path],
            mnemonic = "GoLn",
            inputs=[src],
            outputs=[out],
        )
      else:
        fail("Invalid go path mode '{}'".format(ctx.attr.mode))
    packages += [struct(
      golib = golib,
      dir = _tag(ctx, prefix, outputs),
      files = package_files,
    )]
  gopath = _tag(ctx, "", outputs)
  return [
      DefaultInfo(
          files = depset(outputs),
      ),
      GoPath(
        gopath = gopath,
        packages = packages,
        srcs = outputs,
      )
  ]

go_path = rule(
    _go_path_impl,
    attrs = {
        "deps": attr.label_list(providers=[GoLibrary]),
        "mode": attr.string(default="copy", values=["link", "copy"]),
    },
    toolchains = ["@io_bazel_rules_go//go:toolchain"],
)
