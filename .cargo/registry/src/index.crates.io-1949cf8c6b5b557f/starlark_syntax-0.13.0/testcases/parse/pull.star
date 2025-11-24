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
"""An implementation of container_pull based on google/containerregistry.

This wraps the containerregistry.tools.fast_puller executable in a
Bazel rule for downloading base images without a Docker client to
construct new images.
"""

def _python(repository_ctx):
  if "BAZEL_PYTHON" in repository_ctx.os.environ:
    return repository_ctx.os.environ.get("BAZEL_PYTHON")

  python_path = repository_ctx.which("python")
  if not python_path:
    python_path = repository_ctx.which("python.exe")
  if python_path:
    return python_path

  fail("rules_docker requires a python interpreter installed. " +
       "Please set BAZEL_PYTHON, or put it on your path.")

def _impl(repository_ctx):
  """Core implementation of container_pull."""

  # Add an empty top-level BUILD file.
  repository_ctx.file("BUILD", "")

  repository_ctx.file("image/BUILD", """
package(default_visibility = ["//visibility:public"])

load("@io_bazel_rules_docker//container:import.bzl", "container_import")

container_import(
  name = "image",
  config = "config.json",
  layers = glob(["*.tar.gz"]),
)
""")

  args = [
      _python(repository_ctx),
      repository_ctx.path(repository_ctx.attr._puller),
      "--directory", repository_ctx.path("image")
  ]

  # If a digest is specified, then pull by digest.  Otherwise, pull by tag.
  if repository_ctx.attr.digest:
    args += [
        "--name", "{registry}/{repository}@{digest}".format(
            registry=repository_ctx.attr.registry,
            repository=repository_ctx.attr.repository,
            digest=repository_ctx.attr.digest)
    ]
  else:
    args += [
        "--name", "{registry}/{repository}:{tag}".format(
            registry=repository_ctx.attr.registry,
            repository=repository_ctx.attr.repository,
            tag=repository_ctx.attr.tag)
    ]

  result = repository_ctx.execute(args)
  if result.return_code:
    fail("Pull command failed: %s (%s)" % (result.stderr, " ".join(args)))

container_pull = repository_rule(
    attrs = {
        "registry": attr.string(mandatory = True),
        "repository": attr.string(mandatory = True),
        "digest": attr.string(),
        "tag": attr.string(default = "latest"),
        "_puller": attr.label(
            executable = True,
            default = Label("@puller//file:puller.par"),
            cfg = "host",
        ),
    },
    implementation = _impl,
)

"""Pulls a container image.

This rule pulls a container image into our intermediate format.  The
output of this rule can be used interchangeably with `docker_build`.

Args:
  name: name of the rule.
  registry: the registry from which we are pulling.
  repository: the name of the image.
  tag: (optional) the tag of the image, default to 'latest' if this
       and 'digest' remain unspecified.
  digest: (optional) the digest of the image to pull.
"""
