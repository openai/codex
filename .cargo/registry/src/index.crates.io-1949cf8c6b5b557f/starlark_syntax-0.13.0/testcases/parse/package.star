# @generated
# Copyright (C) 2016 The Android Open Source Project
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
# http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

sh_bang_template = (" && ".join([
    "echo '#!/usr/bin/env bash' > $@",
    "echo \"# this script should run from the root of your workspace.\" >> $@",
    "echo \"set -e\" >> $@",
    "echo \"\" >> $@",
    "echo 'if [[ \"$$VERBOSE\" ]]; then set -x ; fi' >> $@",
    "echo \"\" >> $@",
    "echo %s >> $@",
    "echo \"\" >> $@",
    "echo %s >> $@",
]))

def maven_package(
    version,
    repository = None,
    url = None,
    jar = {},
    src = {},
    doc = {},
    war = {}):

  build_cmd = ['bazel', 'build']
  mvn_cmd = ['python', 'tools/maven/mvn.py', '-v', version]
  api_cmd = mvn_cmd[:]
  api_targets = []
  for type,d in [('jar', jar), ('java-source', src), ('javadoc', doc)]:
    for a,t in sorted(d.items()):
      api_cmd.append('-s %s:%s:$(location %s)' % (a,type,t))
      api_targets.append(t)

  native.genrule(
    name = 'gen_api_install',
    cmd = sh_bang_template % (
      ' '.join(build_cmd + api_targets),
      ' '.join(api_cmd + ['-a', 'install'])),
    srcs = api_targets,
    outs = ['api_install.sh'],
    executable = True,
    testonly = 1,
  )

  if repository and url:
    native.genrule(
      name = 'gen_api_deploy',
      cmd = sh_bang_template % (
        ' '.join(build_cmd + api_targets),
        ' '.join(api_cmd + ['-a', 'deploy',
                            '--repository', repository,
                            '--url', url])),
      srcs = api_targets,
      outs = ['api_deploy.sh'],
      executable = True,
      testonly = 1,
    )

  war_cmd = mvn_cmd[:]
  war_targets = []
  for a,t in sorted(war.items()):
    war_cmd.append('-s %s:war:$(location %s)' % (a,t))
    war_targets.append(t)

  native.genrule(
    name = 'gen_war_install',
    cmd = sh_bang_template % (' '.join(build_cmd + war_targets),
                              ' '.join(war_cmd + ['-a', 'install'])),
    srcs = war_targets,
    outs = ['war_install.sh'],
    executable = True,
  )

  if repository and url:
    native.genrule(
      name = 'gen_war_deploy',
      cmd = sh_bang_template % (
          ' '.join(build_cmd + war_targets),
          ' '.join(war_cmd + [
        '-a', 'deploy',
        '--repository', repository,
        '--url', url])),
      srcs = war_targets,
      outs = ['war_deploy.sh'],
      executable = True,
    )
