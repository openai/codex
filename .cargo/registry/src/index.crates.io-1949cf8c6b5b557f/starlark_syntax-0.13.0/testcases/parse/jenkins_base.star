# @generated
# Copyright 2016 The Bazel Authors. All rights reserved.
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

# Some definition to create a base image for jenkins in docker

load("@io_bazel_rules_docker//docker:docker.bzl", "docker_pull")
JENKINS_PLUGINS_URL = "http://mirrors.xmission.com/jenkins/plugins/{name}/{version}/{name}.hpi"

def _jenkins_image_impl(repository_ctx):
  repository_ctx.file("plugins/BUILD", """
load("@bazel_tools//tools/build_defs/pkg:pkg.bzl", "pkg_tar")
pkg_tar(
    name = "plugins",
    files = glob(["**/*.jpi"]),
    mode = "0644",
    strip_prefix = ".",
    package_dir = "/usr/share/jenkins/ref/plugins",
    visibility = ["//:__pkg__"],
)
""")
  for plugin in repository_ctx.attr.plugins:
    config = repository_ctx.attr.plugins[plugin]
    dest = "plugins/" + plugin + ".jpi"
    repository_ctx.download(
        JENKINS_PLUGINS_URL.format(name=plugin, version=config[0]), dest,
        config[1])
    if len(config) >= 3 and config[2] == "pinned":
      repository_ctx.file(dest + ".pinned", "")
  repository_ctx.file("BUILD", """
load("@io_bazel_rules_docker//docker:docker.bzl", "docker_build")

docker_build(
  name = "image",
  base = "{base}",
  tars = ["//plugins"],
  directory = "/",
  volumes = [{volumes}],
  visibility = ["//visibility:public"],
)
""".format(
  base = repository_ctx.attr.base,
  volumes = ", ".join(['"%s"' % f for f in repository_ctx.attr.volumes])
))

jenkins_image_ = repository_rule(
    implementation = _jenkins_image_impl,
    attrs = {
        "base": attr.string(mandatory=True),
        "plugins": attr.string_list_dict(mandatory=True),
        "volumes": attr.string_list(default=[]),
    })

def jenkins_base(name, plugins, volumes=[], digest=None, version="1.642.4"):
  base = "jenkins_" + version.replace(".", "_")
  if not native.existing_rule(base):
    kwargs = {}
    if digest:
      kwargs["digest"] = digest
    else:
      kwargs["tag"] = version
    docker_pull(
        name = base,
        registry = "index.docker.io",
        repository = "jenkins/jenkins",
        **kwargs
    )
  jenkins_image_(
      name=name,
      plugins=plugins,
      base="@%s//image" % base,
      volumes=volumes)
