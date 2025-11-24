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
"""Tools for dealing with Docker Image layers."""

load(":list.bzl", "reverse")
load(":path.bzl", _get_runfile_path="runfile")

def get_from_target(unused_ctx, target):
  if hasattr(target, "docker_layers"):
    return target.docker_layers
  else:
    # TODO(mattmoor): Use containerregistry.client's FromTarball
    # to create an entry from a tarball base image.
    return []


def assemble(ctx, layers, tags_to_names, output):
  """Create the full image from the list of layers."""
  layers = [l["layer"] for l in layers]
  args = [
      "--output=" + output.path,
  ] + [
      "--tags=" + tag + "=@" + tags_to_names[tag].path
      for tag in tags_to_names
  ] + ["--layer=" + l.path for l in layers]
  inputs = layers + tags_to_names.values()
  ctx.action(
      executable = ctx.executable.join_layers,
      arguments = args,
      inputs = inputs,
      outputs = [output],
      mnemonic = "JoinLayers"
  )


def incremental_load(ctx, layers, images, output):
  """Generate the incremental load statement."""
  ctx.template_action(
      template = ctx.file.incremental_load_template,
      substitutions = {
          "%{load_statements}": "\n".join([
              "incr_load '%s' '%s' '%s'" % (_get_runfile_path(ctx, l["name"]),
                                            _get_runfile_path(ctx, l["id"]),
                                            _get_runfile_path(ctx, l["layer"]))
              # The last layer is the first in the list of layers.
              # We reverse to load the layer from the parent to the child.
              for l in reverse(layers)]),
          "%{tag_statements}": "\n".join([
              "tag_layer '%s' '%s' '%s'" % (
                  img,
                  _get_runfile_path(ctx, images[img]["name"]),
                  _get_runfile_path(ctx, images[img]["id"]))
              for img in images
          ])
      },
      output = output,
      executable = True)


tools = {
    "incremental_load_template": attr.label(
        default=Label("//tools/build_defs/docker:incremental_load_template"),
        single_file=True,
        allow_files=True),
    "join_layers": attr.label(
        default=Label("//tools/build_defs/docker:join_layers"),
        cfg="host",
        executable=True,
        allow_files=True)
}
