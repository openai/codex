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

# Setup jenkins and build the corresponding docker images

load("@io_bazel_rules_docker//docker:docker.bzl", "docker_build")
load(":templates.bzl", "expand_template")

JENKINS_SERVER = "http://jenkins:80"

def jenkins_node(name, remote_fs = "/home/ci", num_executors = 1, mode = "NORMAL",
                 labels = [], docker_base = None, preference = 1,
                 visibility = None):
  """Create a node configuration on Jenkins, with possible docker image.

  Args:
    name: Name of the node on Jenkins.
    remote_fs: path to the home of the Jenkins user.
    num_executors: number of executors (i.e. concurrent build) this machine can have.
    mode: NORMAL for "Utilize this node as much as possible"
      EXCLUSIVE for "Only build jobs with label restrictions matching this node"
    labels: list of Jenkins labels for this node (the node name is always added).
    docker_base: base for the corresponding docker image to create if we should create one
      (if docker_base is not specified, then a corresponding machine should be configured
      to connect to the Jenkins master).
    preference: A preference factor, if a node as a factor of 1 and another a factor of
      4, then the second one will be scheduled 4 time more jobs than the first one.
    visibility: rule visibility.
  """
  native.genrule(
      name = name,
      cmd = """cat >$@ <<'EOF'
<?xml version='1.0' encoding='UTF-8'?>
<slave>
  <name>%s</name>
  <description></description>
  <remoteFS>%s</remoteFS>
  <numExecutors>%s</numExecutors>
  <mode>%s</mode>
  <retentionStrategy class="hudson.slaves.RetentionStrategy$$Always"/>
  <launcher class="hudson.slaves.JNLPLauncher"/>
  <label>%s</label>
  <nodeProperties>
    <jp.ikedam.jenkins.plugins.scoringloadbalancer.preferences.BuildPreferenceNodeProperty plugin="scoring-load-balancer@1.0.1">
      <preference>%s</preference>
    </jp.ikedam.jenkins.plugins.scoringloadbalancer.preferences.BuildPreferenceNodeProperty>
  </nodeProperties>
</slave>
EOF
""" % (name, remote_fs, num_executors, mode, " ".join([name] + labels), preference),
      outs = ["nodes/%s/config.xml" % name],
      visibility = visibility,
      )
  if docker_base:
    # Generate docker image startup script
    expand_template(
        name = name + ".docker-launcher",
        out = name + ".docker-launcher.sh",
        template = "slave_setup.sh",
        substitutions = {
            "NODE_NAME": name,
            "HOME_FS": remote_fs,
            "JENKINS_SERVER": JENKINS_SERVER,
            },
        executable = True,
        )
    # Generate docker image
    docker_build(
        name = name + ".docker",
        base = docker_base,
        volumes = [remote_fs],
        files = [":%s.docker-launcher.sh" % name],
        data_path = ".",
        entrypoint = [
            "/bin/bash",
            "/%s.docker-launcher.sh" % name,
        ],
        visibility = visibility,
        )
