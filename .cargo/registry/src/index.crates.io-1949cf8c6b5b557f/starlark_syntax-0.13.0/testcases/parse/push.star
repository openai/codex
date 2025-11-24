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
"""An implementation of container_push based on google/containerregistry.

This wraps the containerregistry.tools.fast_pusher executable in a
Bazel rule for publishing images.
"""

load(
    "//skylib:path.bzl",
    "runfile",
)
load(
    "//container:layers.bzl",
    _get_layers = "get_from_target",
    _layer_tools = "tools",
)

def _get_runfile_path(ctx, f):
  return "${RUNFILES}/%s" % runfile(ctx, f)

def _impl(ctx):
  """Core implementation of container_push."""
  stamp_inputs = []
  if ctx.attr.stamp:
    stamp_inputs = [ctx.info_file, ctx.version_file]

  image = _get_layers(ctx, ctx.attr.image, ctx.files.image)

  stamp_arg = " ".join(["--stamp-info-file=%s" % _get_runfile_path(ctx, f) for f in stamp_inputs])

  # Leverage our efficient intermediate representation to push.
  legacy_base_arg = ""
  if image.get("legacy"):
    print("Pushing an image based on a tarball can be very " +
          "expensive.  If the image is the output of a " +
          "docker_build, consider dropping the '.tar' extension. " +
          "If the image is checked in, consider using " +
          "docker_import instead.")
    legacy_base_arg = "--tarball=%s" % _get_runfile_path(ctx, image["legacy"])

  blobsums = image.get("blobsum", [])
  digest_arg = " ".join(["--digest=%s" % _get_runfile_path(ctx, f) for f in blobsums])
  blobs = image.get("zipped_layer", [])
  layer_arg = " ".join(["--layer=%s" % _get_runfile_path(ctx, f) for f in blobs])
  config_arg = "--config=%s" % _get_runfile_path(ctx, image["config"])

  ctx.template_action(
      template = ctx.file._tag_tpl,
      substitutions = {
          "%{tag}": "{registry}/{repository}:{tag}".format(
            registry=ctx.expand_make_variables(
              "registry", ctx.attr.registry, {}),
            repository=ctx.expand_make_variables(
              "repository", ctx.attr.repository, {}),
            tag=ctx.expand_make_variables(
              "tag", ctx.attr.tag, {})),
          "%{stamp}": stamp_arg,
          "%{image}": "%s %s %s %s" % (
              legacy_base_arg, config_arg, digest_arg, layer_arg),
          "%{format}": "--oci" if ctx.attr.format == "OCI" else "",
          "%{container_pusher}": _get_runfile_path(ctx, ctx.executable._pusher),
      },
      output = ctx.outputs.executable,
      executable=True,
  )

  return struct(runfiles = ctx.runfiles(files = [
      ctx.executable._pusher,
      image["config"]
  ] + image.get("blobsum", []) + image.get("zipped_layer", []) +
  stamp_inputs + ([image["legacy"]] if image.get("legacy") else []) +
  list(ctx.attr._pusher.default_runfiles.files)))

container_push = rule(
    attrs = {
        "image": attr.label(
            allow_files = [".tar"],
            single_file = True,
            mandatory = True,
        ),
        "registry": attr.string(mandatory = True),
        "repository": attr.string(mandatory = True),
        "tag": attr.string(default = "latest"),
        "format": attr.string(
            mandatory = True,
            values = [
                "OCI",
                "Docker",
            ],
        ),
        "_tag_tpl": attr.label(
            default = Label("//container:push-tag.sh.tpl"),
            single_file = True,
            allow_files = True,
        ),
        "_pusher": attr.label(
            default = Label("@containerregistry//:pusher"),
            cfg = "host",
            executable = True,
            allow_files = True,
        ),
        "stamp": attr.bool(
            default = False,
            mandatory = False,
        ),
    } + _layer_tools,
    executable = True,
    implementation = _impl,
)

"""Pushes a container image.

This rule pushes a container image to a registry.

Args:
  name: name of the rule
  image: the label of the image to push.
  format: The form to push: Docker or OCI.
  registry: the registry to which we are pushing.
  repository: the name of the image.
  tag: (optional) the tag of the image, default to 'latest'.
"""
