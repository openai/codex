# @generated
# pylint: disable=g-bad-file-header
# Copyright 2017 The Bazel Authors. All rights reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#    http:#www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
"""Contains Skylark rules used to build the embedded_tools.zip."""

def _embedded_tools(ctx):
  # The list of arguments we pass to the script.
  args_file = ctx.new_file(ctx.label.name + ".params")
  ctx.file_action(output=args_file, content="\n".join([f.path for f in ctx.files.srcs]))
  # Action to call the script.
  ctx.action(
      inputs=ctx.files.srcs,
      outputs=[ctx.outputs.out],
      arguments=[ctx.outputs.out.path, args_file.path],
      progress_message="Creating embedded tools: %s" % ctx.outputs.out.short_path,
      executable=ctx.executable.tool)

embedded_tools = rule(
    implementation=_embedded_tools,
    attrs={
        "srcs": attr.label_list(allow_files=True),
        "out": attr.output(mandatory=True),
        "tool": attr.label(executable=True, cfg="host", allow_files=True,
                           default=Label("//src:create_embedded_tools_sh"))
    }
)

def _srcsfile(ctx):
  ctx.file_action(
      output=ctx.outputs.out,
      content="\n".join([f.path for f in ctx.files.srcs]))

srcsfile = rule(
    implementation=_srcsfile,
    attrs={
        "srcs": attr.label_list(allow_files=True),
        "out": attr.output(mandatory=True),
    }
)
