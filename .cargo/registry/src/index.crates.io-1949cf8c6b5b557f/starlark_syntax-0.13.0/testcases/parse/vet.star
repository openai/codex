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

load("@io_bazel_rules_go//go/private:providers.bzl", "GoPath")

load("@io_bazel_rules_go//go/private:mode.bzl",
    "get_mode",
)
load("@io_bazel_rules_go//go/private:common.bzl",
    "declare_file",
)

def _go_vet_generate_impl(ctx):
  print("""
EXPERIMENTAL: the go_vet_test rule is still very experimental
Please do not rely on it for production use, but feel free to use it and file issues
""")
  go_toolchain = ctx.toolchains["@io_bazel_rules_go//go:toolchain"]
  mode = get_mode(ctx, ctx.attr._go_toolchain_flags)
  stdlib = go_toolchain.stdlib.get(ctx, go_toolchain, mode)
  script_file = declare_file(ctx, ext=".bash")
  gopath = []
  files = ctx.files.data + stdlib.files
  gopath = []
  packages = []
  for data in ctx.attr.data:
    entry = data[GoPath]
    gopath += [entry.gopath]
    packages += [package.dir for package in entry.packages]
  ctx.actions.write(output=script_file, is_executable=True, content="""
export GOPATH="{gopath}"
{go} tool vet {packages}
""".format(
      go=stdlib.go.short_path,
      gopath=":".join(['$(pwd)/{})'.format(entry) for entry in gopath]),
      packages=" ".join(packages),
  ))
  return struct(
    files = depset([script_file]),
    runfiles = ctx.runfiles(files, collect_data = True),
  )

_go_vet_generate = rule(
    _go_vet_generate_impl,
    attrs = {
        "data": attr.label_list(providers=[GoPath], cfg = "data"),
        "_go_toolchain_flags": attr.label(default=Label("@io_bazel_rules_go//go/private:go_toolchain_flags")),
    },
    toolchains = ["@io_bazel_rules_go//go:toolchain"],
)

def go_vet_test(name, data, **kwargs):
  script_name = "generate_"+name
  _go_vet_generate(
    name=script_name,
    data=data,
    tags = ["manual"],
  )
  native.sh_test(
    name=name,
    srcs=[script_name],
    data=data,
    **kwargs
  )