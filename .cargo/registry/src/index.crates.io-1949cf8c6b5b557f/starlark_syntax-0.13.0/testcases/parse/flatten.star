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
"""A rule to flatten container images."""

load(
    "//skylib:path.bzl",
    _get_runfile_path = "runfile",
)
load(
    "//container:layers.bzl",
    _get_layers = "get_from_target",
    _layer_tools = "tools",
)

def _impl(ctx):
  """Core implementation of container_flatten."""

  image = _get_layers(ctx, ctx.attr.image, ctx.files.image)

  # Leverage our efficient intermediate representation to push.
  legacy_base_arg = []
  legacy_files = []
  if image.get("legacy"):
    # TODO(mattmoor): warn about legacy base.
    legacy_files += [image["legacy"]]
    legacy_base_arg = ["--tarball=%s" % image["legacy"].path]

  blobsums = image.get("blobsum", [])
  digest_args = ["--digest=" + f.path for f in blobsums]
  blobs = image.get("zipped_layer", [])
  layer_args = ["--layer=" + f.path for f in blobs]
  config_arg = "--config=%s" % image["config"].path

  ctx.action(
      executable = ctx.executable._flattener,
      arguments = legacy_base_arg + digest_args + layer_args + [
          config_arg,
          "--filesystem=" + ctx.outputs.filesystem.path,
          "--metadata=" + ctx.outputs.metadata.path,
      ],
      inputs = blobsums + blobs + [image["config"]] + legacy_files,
      outputs = [ctx.outputs.filesystem, ctx.outputs.metadata],
      use_default_shell_env=True,
      mnemonic="Flatten"
  )
  return struct()

container_flatten = rule(
    attrs = {
        "image": attr.label(
            allow_files = [".tar"],
            single_file = True,
            mandatory = True,
        ),
        "_flattener": attr.label(
            default = Label("@containerregistry//:flatten"),
            cfg = "host",
            executable = True,
            allow_files = True,
        ),
    } + _layer_tools,
    outputs = {
        "filesystem": "%{name}.tar",
        "metadata": "%{name}.json",
    },
    implementation = _impl,
)
