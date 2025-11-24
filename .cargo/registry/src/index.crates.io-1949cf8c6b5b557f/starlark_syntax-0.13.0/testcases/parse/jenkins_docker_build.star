# @generated
# Copyright 2015 The Bazel Authors. All rights reserved.
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

# Creation of the docker container for the jenkins master.

load("@io_bazel_rules_docker//docker:docker.bzl", "docker_build")
load(":templates.bzl", "merge_files", "strip_suffix")
load(":vars.bzl", "MAIL_SUBSTITUTIONS")

def _build_jobs_impl(ctx):
  output = ctx.outputs.out
  folders_to_create = {}
  args = [
    "--output=" + output.path,
    "--mode=0644",
    "--directory=/usr/share/jenkins/ref/jobs",
    ]
  suffixes = ctx.attr.strip_suffixes
  # Group fob by folders
  for f in ctx.files.jobs:
    if f.owner and "/" in f.owner.name:
      segments = f.owner.name.split("/")
      for i in range(1, len(segments)):
        folders_to_create["/jobs/".join(segments[:i])] = True
      p = strip_suffix(f.owner.name.replace("/", "/jobs/"), suffixes)
      args.append("--file=%s=%s/config.xml" % (f.path, p))
    else:
      p = strip_suffix(f.basename[:-len(f.extension)-1], suffixes)
      args.append("--file=%s=%s/config.xml" % (f.path, p))

  for folder in folders_to_create:
    args.append("--file=%s=%s/config.xml" % (ctx.file._folder_xml.path, folder))

  ctx.action(
      executable = ctx.executable._build_tar,
      arguments = args,
      inputs = ctx.files.jobs + [ctx.file._folder_xml],
      outputs = [output],
      mnemonic="TarJobs"
      )

_build_jobs = rule(
    attrs = {
        "jobs": attr.label_list(allow_files=True),
        "strip_suffixes": attr.string_list(default=["-staging", "-test"]),
        "_folder_xml": attr.label(
            default=Label("//jenkins/build_defs:folder.xml"),
            allow_files=True,
            single_file=True),
        "_build_tar": attr.label(
            default=Label("@bazel_tools//tools/build_defs/pkg:build_tar"),
            cfg="host",
            executable=True,
            allow_files=True),
    },
    outputs = {"out": "%{name}.tar"},
    implementation = _build_jobs_impl,
)


def jenkins_docker_build(name, plugins = None, base = "//jenkins/base", configs = [],
                  jobs = [], substitutions = {}, visibility = None, tars = []):
  """Build the docker image for the Jenkins instance."""
  substitutions = substitutions + MAIL_SUBSTITUTIONS
  # Expands config files in a tar ball
  merge_files(
      name = "%s-configs" % name,
      srcs = configs,
      directory = "/usr/share/jenkins/ref",
      strip_prefixes = [
          "jenkins/config",
          "jenkins",
      ],
      substitutions = substitutions)
  tars += ["%s-configs" % name]

  # Create the structures for jobs
  _build_jobs(name=name + "-jobs", jobs=jobs)
  tars += ["%s-jobs" % name]

  ### FINAL IMAGE ###
  docker_build(
      name = name,
      tars = tars,
      # Workaround no way to specify owner in pkg_tar
      # TODO(dmarting): use https://cr.bazel.build/10255 when it hits a release.
      user = "root",
      entrypoint = [
          "/bin/tini",
          "--",
          "/bin/bash",
          "-c",
          "[ -d /opt/lib ] && chown -R jenkins /opt/lib; su jenkins -c /usr/local/bin/jenkins.sh",
      ],
      # End of workaround
      base = base,
      directory = "/",
      visibility = visibility,
  )
