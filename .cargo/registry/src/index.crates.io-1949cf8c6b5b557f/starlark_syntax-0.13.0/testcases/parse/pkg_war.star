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

# War packaging.

jar_filetype = FileType([".jar"])

LIBS = [
    "//gerrit-war:init",
    "//gerrit-war:log4j-config",
    "//gerrit-war:version",
    "//lib:postgresql",
    "//lib/bouncycastle:bcpkix",
    "//lib/bouncycastle:bcprov",
    "//lib/bouncycastle:bcpg",
    "//lib/log:impl_log4j",
]

PGMLIBS = [
    "//gerrit-pgm:pgm",
]

def _add_context(in_file, output):
  input_path = in_file.path
  return [
    'unzip -qd %s %s' % (output, input_path)
  ]

def _add_file(in_file, output):
  output_path = output
  input_path = in_file.path
  short_path = in_file.short_path
  n = in_file.basename

  if short_path.startswith('gerrit-'):
    n = short_path.split('/')[0] + '-' + n

  output_path += n
  return [
    'test -L %s || ln -s $(pwd)/%s %s' % (output_path, input_path, output_path)
  ]

def _make_war(input_dir, output):
  return '(%s)' % ' && '.join([
    'root=$(pwd)',
    'cd %s' % input_dir,
    "find . -exec touch -t 198001010000 '{}' ';' 2> /dev/null",
    'zip -9qr ${root}/%s .' % (output.path),
  ])

def _war_impl(ctx):
  war = ctx.outputs.war
  build_output = war.path + '.build_output'
  inputs = []

  # Create war layout
  cmd = [
    'set -e;rm -rf ' + build_output,
    'mkdir -p ' + build_output,
    'mkdir -p %s/WEB-INF/lib' % build_output,
    'mkdir -p %s/WEB-INF/pgm-lib' % build_output,
  ]

  # Add lib
  transitive_lib_deps = set_which_is_banned()
  for l in ctx.attr.libs:
    if hasattr(l, 'java'):
      transitive_lib_deps += l.java.transitive_runtime_deps
    elif hasattr(l, 'files'):
      transitive_lib_deps += l.files

  for dep in transitive_lib_deps:
    cmd += _add_file(dep, build_output + '/WEB-INF/lib/')
    inputs.append(dep)

  # Add pgm lib
  transitive_pgmlib_deps = set_which_is_banned()
  for l in ctx.attr.pgmlibs:
    transitive_pgmlib_deps += l.java.transitive_runtime_deps

  for dep in transitive_pgmlib_deps:
    if dep not in inputs:
      cmd += _add_file(dep, build_output + '/WEB-INF/pgm-lib/')
      inputs.append(dep)

  # Add context
  transitive_context_deps = set_which_is_banned()
  if ctx.attr.context:
    for jar in ctx.attr.context:
      if hasattr(jar, 'java'):
        transitive_context_deps += jar.java.transitive_runtime_deps
      elif hasattr(jar, 'files'):
        transitive_context_deps += jar.files
  for dep in transitive_context_deps:
    cmd += _add_context(dep, build_output)
    inputs.append(dep)

  # Add zip war
  cmd.append(_make_war(build_output, war))

  ctx.action(
    inputs = inputs,
    outputs = [war],
    mnemonic = 'WAR',
    command = '\n'.join(cmd),
    use_default_shell_env = True,
  )

# context: go to the root directory
# libs: go to the WEB-INF/lib directory
# pgmlibs: go to the WEB-INF/pgm-lib directory
_pkg_war = rule(
    attrs = {
        "context": attr.label_list(allow_files = True),
        "libs": attr.label_list(allow_files = jar_filetype),
        "pgmlibs": attr.label_list(allow_files = False),
    },
    outputs = {"war": "%{name}.war"},
    implementation = _war_impl,
)

def pkg_war(name, ui = 'ui_optdbg', context = [], doc = False, **kwargs):
  doc_ctx = []
  doc_lib = []
  ui_deps = []
  if ui == 'polygerrit' or ui == 'ui_optdbg' or ui == 'ui_optdbg_r':
    ui_deps.append('//polygerrit-ui/app:polygerrit_ui')
  if ui and ui != 'polygerrit':
    ui_deps.append('//gerrit-gwtui:%s' % ui)
  if doc:
    doc_ctx.append('//Documentation:html')
    doc_lib.append('//Documentation:index')

  _pkg_war(
    name = name,
    libs = LIBS + doc_lib,
    pgmlibs = PGMLIBS,
    context = doc_ctx + context + ui_deps + [
      '//gerrit-main:main_bin_deploy.jar',
      '//gerrit-war:webapp_assets',
    ],
    **kwargs
  )
