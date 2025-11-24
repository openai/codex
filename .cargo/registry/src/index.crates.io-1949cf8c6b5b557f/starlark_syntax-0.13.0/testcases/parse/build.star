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
"""Rule for building a Docker image."""

load(":filetype.bzl",
     tar_filetype="tar",
     deb_filetype="deb",
     docker_filetype="docker")
load("//tools/build_defs/hash:hash.bzl",
     _hash_tools="tools", _sha256="sha256")
load(":label.bzl", _string_to_label="string_to_label")
load(":layers.bzl",
     _assemble_image="assemble",
     _get_layers="get_from_target",
     _incr_load="incremental_load",
     _layer_tools="tools")
load(":list.bzl", "reverse")
load(":path.bzl",
     "dirname", "strip_prefix",
     _join_path="join",
     _canonicalize_path="canonicalize")
load(":serialize.bzl", _serialize_dict="dict_to_associative_list")


def _build_layer(ctx):
  """Build the current layer for appending it the base layer."""

  layer = ctx.new_file(ctx.label.name + ".layer")
  build_layer = ctx.executable.build_layer
  args = [
      "--output=" + layer.path,
      "--directory=" + ctx.attr.directory,
      "--mode=" + ctx.attr.mode,
  ]

  if ctx.attr.data_path:
    # If data_prefix is specified, then add files relative to that.
    data_path = _join_path(
        dirname(ctx.outputs.out.short_path),
        _canonicalize_path(ctx.attr.data_path))
    args += ["--file=%s=%s" % (f.path, strip_prefix(f.short_path, data_path))
             for f in ctx.files.files]
  else:
    # Otherwise, files are added without a directory prefix at all.
    args += ["--file=%s=%s" % (f.path, f.basename)
             for f in ctx.files.files]

  args += ["--tar=" + f.path for f in ctx.files.tars]
  args += ["--deb=" + f.path for f in ctx.files.debs if f.path.endswith(".deb")]
  args += ["--link=%s:%s" % (k, ctx.attr.symlinks[k])
           for k in ctx.attr.symlinks]
  arg_file = ctx.new_file(ctx.label.name + ".layer.args")
  ctx.file_action(arg_file, "\n".join(args))

  ctx.action(
      executable = build_layer,
      arguments = ["--flagfile=" + arg_file.path],
      inputs = ctx.files.files + ctx.files.tars + ctx.files.debs + [arg_file],
      outputs = [layer],
      use_default_shell_env=True,
      mnemonic="DockerLayer"
  )
  return layer


# TODO(mattmoor): In a future change, we should establish the invariant that
# base must expose "docker_layers", possibly by hoisting a "docker_load" rule
# from a tarball "base".
def _get_base_artifact(ctx):
  if ctx.files.base:
    if hasattr(ctx.attr.base, "docker_layers"):
      # The base is the first layer in docker_layers if provided.
      return _get_layers(ctx, ctx.attr.base)[0]["layer"]
    if len(ctx.files.base) != 1:
      fail("base attribute should be a single tar file.")
    return ctx.files.base[0]


def _image_config(ctx, layer_names):
  """Create the configuration for a new docker image."""
  config = ctx.new_file(ctx.label.name + ".config")

  label_file_dict = _string_to_label(
      ctx.files.label_files, ctx.attr.label_file_strings)

  labels = dict()
  for l in ctx.attr.labels:
    fname = ctx.attr.labels[l]
    if fname[0] == "@":
      labels[l] = "@" + label_file_dict[fname[1:]].path
    else:
      labels[l] = fname

  args = [
      "--output=%s" % config.path,
      "--entrypoint=%s" % ",".join(ctx.attr.entrypoint),
      "--command=%s" % ",".join(ctx.attr.cmd),
      "--labels=%s" % _serialize_dict(labels),
      "--env=%s" % _serialize_dict(ctx.attr.env),
      "--ports=%s" % ",".join(ctx.attr.ports),
      "--volumes=%s" % ",".join(ctx.attr.volumes)
  ]
  if ctx.attr.user:
    args += ["--user=" + ctx.attr.user]
  if ctx.attr.workdir:
    args += ["--workdir=" + ctx.attr.workdir]

  inputs = layer_names
  args += ["--layer=@" + l.path for l in layer_names]

  if ctx.attr.label_files:
    inputs += ctx.files.label_files

  base = _get_base_artifact(ctx)
  if base:
    args += ["--base=%s" % base.path]
    inputs += [base]

  ctx.action(
      executable = ctx.executable.create_image_config,
      arguments = args,
      inputs = inputs,
      outputs = [config],
      use_default_shell_env=True,
      mnemonic = "ImageConfig")
  return config


def _metadata_action(ctx, layer, name, output):
  """Generate the action to create the JSON metadata for the layer."""
  rewrite_tool = ctx.executable.rewrite_tool

  label_file_dict = _string_to_label(
      ctx.files.label_files, ctx.attr.label_file_strings)

  labels = dict()
  for l in ctx.attr.labels:
    fname = ctx.attr.labels[l]
    if fname[0] == "@":
      labels[l] = "@" + label_file_dict[fname[1:]].path
    else:
      labels[l] = fname

  args = [
      "--output=%s" % output.path,
      "--layer=%s" % layer.path,
      "--name=@%s" % name.path,
      "--entrypoint=%s" % ",".join(ctx.attr.entrypoint),
      "--command=%s" % ",".join(ctx.attr.cmd),
      "--labels=%s" % _serialize_dict(labels),
      "--env=%s" % _serialize_dict(ctx.attr.env),
      "--ports=%s" % ",".join(ctx.attr.ports),
      "--volumes=%s" % ",".join(ctx.attr.volumes)
  ]
  if ctx.attr.workdir:
    args += ["--workdir=" + ctx.attr.workdir]
  inputs = [layer, rewrite_tool, name]
  if ctx.attr.label_files:
    inputs += ctx.files.label_files

  # TODO(mattmoor): Does this properly handle naked tarballs?
  base = _get_base_artifact(ctx)
  if base:
    args += ["--base=%s" % base.path]
    inputs += [base]
  if ctx.attr.user:
    args += ["--user=" + ctx.attr.user]

  ctx.action(
      executable = rewrite_tool,
      arguments = args,
      inputs = inputs,
      outputs = [output],
      use_default_shell_env=True,
      mnemonic = "RewriteJSON")


def _metadata(ctx, layer, name):
  """Create the metadata for the new docker image."""
  metadata = ctx.new_file(ctx.label.name + ".metadata")
  _metadata_action(ctx, layer, name, metadata)
  return metadata


def _compute_layer_name(ctx, layer):
  """Compute the layer's name.

  This function synthesize a version of its metadata where in place
  of its final name, we use the SHA256 of the layer blob.

  This makes the name of the layer a function of:
    - Its layer's SHA256
    - Its metadata
    - Its parent's name.
  Assuming the parent's name is derived by this same rigor, then
  a simple induction proves the content addressability.

  Args:
    ctx: Rule context.
    layer: The layer's artifact for which to compute the name.
  Returns:
    The artifact that will contains the name for the layer.
  """
  metadata = ctx.new_file(ctx.label.name + ".metadata-name")
  layer_sha = _sha256(ctx, layer)
  _metadata_action(ctx, layer, layer_sha, metadata)
  return _sha256(ctx, metadata)


def _repository_name(ctx):
  """Compute the repository name for the current rule."""
  if ctx.attr.legacy_repository_naming:
    # Legacy behavior, off by default.
    return _join_path(ctx.attr.repository, ctx.label.package.replace("/", "_"))
  # Newer Docker clients support multi-level names, which are a part of
  # the v2 registry specification.
  return _join_path(ctx.attr.repository, ctx.label.package)


def _create_image(ctx, layers, identifier, config, name, metadata, tags):
  """Create the new image."""
  args = [
      "--output=" + ctx.outputs.layer.path,
      "--id=@" + identifier.path,
      "--config=" + config.path,
  ] + ["--tag=" + tag for tag in tags]

  args += ["--layer=@%s=%s" % (l["name"].path, l["layer"].path) for l in layers]
  inputs = [identifier, config] + [l["name"] for l in layers] + [l["layer"] for l in layers]

  if name:
    args += ["--legacy_id=@" + name.path]
    inputs += [name]

  if metadata:
    args += ["--metadata=" + metadata.path]
    inputs += [metadata]

  # If we have been provided a base image, add it.
  if ctx.attr.base and not hasattr(ctx.attr.base, "docker_layers"):
    legacy_base = _get_base_artifact(ctx)
    if legacy_base:
      args += ["--legacy_base=%s" % legacy_base.path]
      inputs += [legacy_base]

  # TODO(mattmoor): Does this properly handle naked tarballs? (excl. above)
  base = _get_base_artifact(ctx)
  if base:
    args += ["--base=%s" % base.path]
    inputs += [base]
  ctx.action(
      executable = ctx.executable.create_image,
      arguments = args,
      inputs = inputs,
      outputs = [ctx.outputs.layer],
      mnemonic = "CreateImage",
  )


def _docker_build_impl(ctx):
  """Implementation for the docker_build rule."""
  layer = _build_layer(ctx)
  layer_sha = _sha256(ctx, layer)

  config = _image_config(ctx, [layer_sha])
  identifier = _sha256(ctx, config)

  name = _compute_layer_name(ctx, layer)
  metadata = _metadata(ctx, layer, name)

  # Construct a temporary name based on the build target.
  tags = [_repository_name(ctx) + ":" + ctx.label.name]

  # creating a partial image so only pass the layers that belong to it
  image_layer = {"layer": layer, "name": layer_sha}
  _create_image(ctx, [image_layer], identifier, config, name, metadata, tags)

  # Compute the layers transitive provider.
  # This must includes all layers of the image, including:
  #  - The layer introduced by this rule.
  #  - The layers transitively introduced by docker_build deps.
  #  - Layers introduced by a static tarball base.
  # This is because downstream tooling should just be able to depend on
  # the availability and completeness of this field.
  layers =  [
      {"layer": ctx.outputs.layer, "id": identifier, "name": name}
  ] + _get_layers(ctx, ctx.attr.base)

  # Generate the incremental load statement
  _incr_load(ctx, layers, {tag_name: {"name": name, "id": identifier}
                           for tag_name in tags},
             ctx.outputs.executable)

  _assemble_image(ctx, reverse(layers), {tag_name: name for tag_name in tags},
                  ctx.outputs.out)
  runfiles = ctx.runfiles(
      files = ([l["name"] for l in layers] +
               [l["id"] for l in layers] +
               [l["layer"] for l in layers]))
  return struct(runfiles = runfiles,
                files = depset([ctx.outputs.layer]),
                docker_layers = layers)


docker_build_ = rule(
    implementation = _docker_build_impl,
    attrs = {
        "base": attr.label(allow_files=docker_filetype),
        "data_path": attr.string(),
        "directory": attr.string(default="/"),
        "tars": attr.label_list(allow_files=tar_filetype),
        "debs": attr.label_list(allow_files=deb_filetype),
        "files": attr.label_list(allow_files=True),
        "legacy_repository_naming": attr.bool(default=False),
        "mode": attr.string(default="0555"),
        "symlinks": attr.string_dict(),
        "entrypoint": attr.string_list(),
        "cmd": attr.string_list(),
        "user": attr.string(),
        "env": attr.string_dict(),
        "labels": attr.string_dict(),
        "ports": attr.string_list(),  # Skylark doesn't support int_list...
        "volumes": attr.string_list(),
        "workdir": attr.string(),
        "repository": attr.string(default="bazel"),
        # Implicit dependencies.
        "label_files": attr.label_list(
            allow_files=True),
        "label_file_strings": attr.string_list(),
        "build_layer": attr.label(
            default=Label("//tools/build_defs/pkg:build_tar"),
            cfg="host",
            executable=True,
            allow_files=True),
        "create_image": attr.label(
            default=Label("//tools/build_defs/docker:create_image"),
            cfg="host",
            executable=True,
            allow_files=True),
        "rewrite_tool": attr.label(
            default=Label("//tools/build_defs/docker:rewrite_json"),
            cfg="host",
            executable=True,
            allow_files=True),
        "create_image_config": attr.label(
            default=Label("//tools/build_defs/docker:create_image_config"),
            cfg="host",
            executable=True,
            allow_files=True)
    } + _hash_tools + _layer_tools,
    outputs = {
        "out": "%{name}.tar",
        "layer": "%{name}-layer.tar",
    },
    executable = True)


# This validates the two forms of value accepted by
# ENTRYPOINT and CMD, turning them into a canonical
# python list form.
#
# The Dockerfile construct:
#   ENTRYPOINT "/foo"
# Results in:
#   "Entrypoint": [
#       "/bin/sh",
#       "-c",
#       "\"/foo\""
#   ],
# Whereas:
#   ENTRYPOINT ["/foo", "a"]
# Results in:
#   "Entrypoint": [
#       "/foo",
#       "a"
#   ],
# NOTE: prefacing a command with 'exec' just ends up with the former
def _validate_command(name, argument):
  if type(argument) == "string":
    return ["/bin/sh", "-c", argument]
  elif type(argument) == "list":
    return argument
  elif argument:
    fail("The %s attribute must be a string or list, if specified." % name)
  else:
    return None


# Produces a new docker image tarball compatible with 'docker load', which
# is a single additional layer atop 'base'.  The goal is to have relatively
# complete support for building docker image, from the Dockerfile spec.
#
# For more information see the 'Config' section of the image specification:
# https://github.com/opencontainers/image-spec/blob/v0.2.0/serialization.md
#
# Only 'name' is required. All other fields have sane defaults.
#
#   docker_build(
#      name="...",
#      visibility="...",
#
#      # The base layers on top of which to overlay this layer,
#      # equivalent to FROM.
#      base="//another/build:rule",
#
#      # The base directory of the files, defaulted to
#      # the package of the input.
#      # All files structure relatively to that path will be preserved.
#      # A leading '/' mean the workspace root and this path is relative
#      # to the current package by default.
#      data_path="...",
#
#      # The directory in which to expand the specified files,
#      # defaulting to '/'.
#      # Only makes sense accompanying one of files/tars/debs.
#      directory="...",
#
#      # The set of archives to expand, or packages to install
#      # within the chroot of this layer
#      files=[...],
#      tars=[...],
#      debs=[...],
#
#      # The set of symlinks to create within a given layer.
#      symlinks = {
#          "/path/to/link": "/path/to/target",
#          ...
#      },
#
#      # https://docs.docker.com/reference/builder/#entrypoint
#      entrypoint="...", or
#      entrypoint=[...],            -- exec form
#
#      # https://docs.docker.com/reference/builder/#cmd
#      cmd="...", or
#      cmd=[...],                   -- exec form
#
#      # https://docs.docker.com/reference/builder/#expose
#      ports=[...],
#
#      # https://docs.docker.com/reference/builder/#user
#      # NOTE: the normal directive affects subsequent RUN, CMD,
#      # and ENTRYPOINT
#      user="...",
#
#      # https://docs.docker.com/reference/builder/#volume
#      volumes=[...],
#
#      # https://docs.docker.com/reference/builder/#workdir
#      # NOTE: the normal directive affects subsequent RUN, CMD,
#      # ENTRYPOINT, ADD, and COPY, but this attribute only affects
#      # the entry point.
#      workdir="...",
#
#      # https://docs.docker.com/reference/builder/#env
#      env = {
#         "var1": "val1",
#         "var2": "val2",
#         ...
#         "varN": "valN",
#      },
#   )
def docker_build(**kwargs):
  """Package a docker image.

  This rule generates a sequence of genrules the last of which is named 'name',
  so the dependency graph works out properly.  The output of this rule is a
  tarball compatible with 'docker save/load' with the structure:
    {layer-name}:
      layer.tar
      VERSION
      json
    {image-config-sha256}.json
    ...
    manifest.json
    repositories
    top     # an implementation detail of our rules, not consumed by Docker.
  This rule appends a single new layer to the tarball of this form provided
  via the 'base' parameter.

  The images produced by this rule are always named 'bazel/tmp:latest' when
  loaded (an internal detail).  The expectation is that the images produced
  by these rules will be uploaded using the 'docker_push' rule below.

  Args:
    **kwargs: See above.
  """
  if "cmd" in kwargs:
    kwargs["cmd"] = _validate_command("cmd", kwargs["cmd"])
  for reserved in ["label_files", "label_file_strings"]:
    if reserved in kwargs:
      fail("reserved for internal use by docker_build macro", attr=reserved)
  if "labels" in kwargs:
    files = sorted(depset([v[1:] for v in kwargs["labels"].values() if v[0] == "@"]))
    kwargs["label_files"] = files
    kwargs["label_file_strings"] = files
  if "entrypoint" in kwargs:
    kwargs["entrypoint"] = _validate_command("entrypoint", kwargs["entrypoint"])
  docker_build_(**kwargs)
