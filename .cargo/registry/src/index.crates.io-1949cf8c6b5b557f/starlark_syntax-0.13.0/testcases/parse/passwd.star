# @generated
# Copyright 2017 The Bazel Authors. All rights reserved.
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

load("@bazel_tools//tools/build_defs/pkg:pkg.bzl", "pkg_tar")

def _impl(ctx):
  """Core implementation of passwd_file."""

  f = "%s:x:%s:%s:%s:%s:%s\n" % (
      ctx.attr.username,
      ctx.attr.uid,
      ctx.attr.gid,
      ctx.attr.info,
      ctx.attr.home,
      ctx.attr.shell
  )
  ctx.file_action(
      output = ctx.outputs.out,
      content = f,
      executable=False
  )
  build_tar = ctx.executable.build_tar
  args = [
      "--output=" + ctx.outputs.tar.path,
      "--file=%s=/etc/passwd" % ctx.outputs.out.path
  ]
  arg_file = ctx.new_file(ctx.attr.name + ".args")
  ctx.file_action(arg_file, "\n".join(args))

  ctx.action(
      executable = build_tar,
      arguments = ["--flagfile=" + arg_file.path],
      inputs = [ctx.outputs.out, arg_file],
      outputs = [ctx.outputs.tar],
      use_default_shell_env = True
  )

passwd_file = rule(
    attrs = {
        "username": attr.string(mandatory = True),
        "uid": attr.int(default = 1000),
        "gid": attr.int(default = 1000),
        "info": attr.string(default = "user"),
        "home": attr.string(default = "/home"),
        "shell": attr.string(default = "/bin/bash"),
        "build_tar": attr.label(
            default = Label("@bazel_tools//tools/build_defs/pkg:build_tar"),
            cfg = "host",
            executable = True,
            allow_files = True,
        ),
    },
    executable = False,
    outputs = {
        "out": "%{name}.passwd",
        "tar": "%{name}.passwd.tar",
    },
    implementation = _impl,
)
