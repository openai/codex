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
"""Rule for bundling Docker images into a tarball."""

load(":label.bzl", _string_to_label="string_to_label")
load(":layers.bzl",
     _assemble_image="assemble",
     _get_layers="get_from_target",
     _incr_load="incremental_load",
     _layer_tools="tools")
load(":list.bzl", "reverse")


def _docker_bundle_impl(ctx):
  """Implementation for the docker_bundle rule."""

  # Compute the set of layers from the image_targes.
  image_target_dict = _string_to_label(
      ctx.attr.image_targets, ctx.attr.image_target_strings)

  seen_names = []
  layers = []
  for image in ctx.attr.image_targets:
    # TODO(mattmoor): Add support for naked tarballs.
    for layer in _get_layers(ctx, image):
      if layer["name"].path in seen_names:
        continue
      seen_names.append(layer["name"].path)
      layers.append(layer)

  images = dict()
  for unresolved_tag in ctx.attr.images:
    # Allow users to put make variables into the tag name.
    tag = ctx.expand_make_variables("images", unresolved_tag, {})

    target = ctx.attr.images[unresolved_tag]
    target = image_target_dict[target]
    images[tag] = _get_layers(ctx, target)[0]

  _incr_load(ctx, layers, images, ctx.outputs.executable)

  _assemble_image(ctx, reverse(layers), {
      # Create a new dictionary with the same keyspace that
      # points to the name of the layer.
      k: images[k]["name"]
      for k in images
  }, ctx.outputs.out)

  runfiles = ctx.runfiles(
      files = ([l["name"] for l in layers] +
               [l["id"] for l in layers] +
               [l["layer"] for l in layers]))

  return struct(runfiles = runfiles,
                files = depset())


docker_bundle_ = rule(
    implementation = _docker_bundle_impl,
    attrs = {
        "images": attr.string_dict(),
        # Implicit dependencies.
        "image_targets": attr.label_list(allow_files=True),
        "image_target_strings": attr.string_list(),
    } + _layer_tools,
    outputs = {
        "out": "%{name}.tar",
    },
    executable = True)


# Produces a new docker image tarball compatible with 'docker load', which
# contains the N listed 'images', each aliased with their key.
#
# Example:
#   docker_bundle(
#     name = "foo",
#     images = {
#       "ubuntu:latest": ":blah",
#       "foo.io/bar:canary": "//baz:asdf",
#     }
#   )
def docker_bundle(**kwargs):
  """Package several docker images into a single tarball.

  Args:
    **kwargs: See above.
  """
  for reserved in ["image_targets", "image_target_strings"]:
    if reserved in kwargs:
      fail("reserved for internal use by docker_bundle macro", attr=reserved)

  if "images" in kwargs:
    kwargs["image_targets"] = kwargs["images"].values()
    kwargs["image_target_strings"] = kwargs["images"].values()

  docker_bundle_(**kwargs)
