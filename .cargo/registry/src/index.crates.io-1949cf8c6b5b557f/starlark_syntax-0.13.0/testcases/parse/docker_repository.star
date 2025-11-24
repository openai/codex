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
"""Rule for importing the docker binary for tests (experimental)."""

def _impl(ctx):
  docker = ctx.which("docker")
  if docker == None:
    # We cannot find docker, we won't be able to run tests depending
    # on it, silently ignoring.
    ctx.file("BUILD",
             "\n".join([
                 "filegroup(",
                 "    name = 'docker',",
                 "    visibility = ['//visibility:public'],",
                 ")"
                 ]))
  else:
    exports = []
    for k in ctx.os.environ:
      # DOCKER* environment variable are used by the docker client
      # to know how to talk to the docker daemon.
      if k.startswith("DOCKER"):
        exports.append("export %s='%s'" % (k, ctx.os.environ[k]))
    ctx.symlink(docker, "docker-bin")
    ctx.file("docker.sh", "\n".join([
        "#!/bin/bash",
        "\n".join(exports),
"""BIN="$0"
while [ -L "${BIN}" ]; do
  BIN="$(readlink "${BIN}")"
done
exec "${BIN%%.sh}-bin" "$@"
"""]))
    ctx.file("BUILD", "\n".join([
        "sh_binary(",
        "    name = 'docker',",
        "    srcs = ['docker.sh'],",
        "    data = [':docker-bin'],",
        "    visibility = ['//visibility:public'],",
        ")"]))

docker_repository_ = repository_rule(_impl)

def docker_repository():
  """Declare a @docker repository that provide a docker binary."""
  docker_repository_(name = "docker")
