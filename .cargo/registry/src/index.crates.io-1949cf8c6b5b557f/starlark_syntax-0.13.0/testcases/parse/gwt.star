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

# Port of Buck native gwt_binary() rule. See discussion in context of
# https://github.com/facebook/buck/issues/109
load("//tools/bzl:genrule2.bzl", "genrule2")
load("//tools/bzl:java.bzl", "java_library2")

jar_filetype = FileType([".jar"])

BROWSERS = [
    "chrome",
    "firefox",
    "gecko1_8",
    "safari",
    "msie",
    "ie8",
    "ie9",
    "ie10",
    "edge",
]

ALIASES = {
    "chrome": "safari",
    "firefox": "gecko1_8",
    "msie": "ie10",
    "edge": "gecko1_8",
}

MODULE = "com.google.gerrit.GerritGwtUI"

GWT_COMPILER = "com.google.gwt.dev.Compiler"

GWT_JVM_ARGS = ["-Xmx512m"]

GWT_COMPILER_ARGS = [
    "-XdisableClassMetadata",
]

GWT_COMPILER_ARGS_RELEASE_MODE = GWT_COMPILER_ARGS + [
    "-XdisableCastChecking",
]

PLUGIN_DEPS_NEVERLINK = [
    "//gerrit-plugin-api:lib-neverlink",
]

GWT_PLUGIN_DEPS_NEVERLINK = [
    "//gerrit-plugin-gwtui:gwtui-api-lib-neverlink",
    "//lib/gwt:user-neverlink",
]

GWT_PLUGIN_DEPS = [
    "//gerrit-plugin-gwtui:gwtui-api-lib",
]

GWT_TRANSITIVE_DEPS = [
    "//lib/gwt:ant",
    "//lib/gwt:colt",
    "//lib/gwt:javax-validation",
    "//lib/gwt:javax-validation_src",
    "//lib/gwt:jsinterop-annotations",
    "//lib/gwt:jsinterop-annotations_src",
    "//lib/gwt:tapestry",
    "//lib/gwt:w3c-css-sac",
    "//lib/ow2:ow2-asm",
    "//lib/ow2:ow2-asm-analysis",
    "//lib/ow2:ow2-asm-commons",
    "//lib/ow2:ow2-asm-tree",
    "//lib/ow2:ow2-asm-util",
]

DEPS = GWT_TRANSITIVE_DEPS + [
    "//gerrit-gwtexpui:CSS",
    "//lib:gwtjsonrpc",
    "//lib/gwt:dev",
    "//lib/jgit/org.eclipse.jgit:jgit-source",
]

USER_AGENT_XML = """<module rename-to='gerrit_ui'>
<inherits name='%s'/>
<set-property name='user.agent' value='%s'/>
<set-property name='locale' value='default'/>
</module>
"""

def gwt_module(gwt_xml=None, resources=[], srcs=[], **kwargs):
  if gwt_xml:
    resources += [gwt_xml]

  java_library2(
    srcs = srcs,
    resources = resources,
    **kwargs)

def _gwt_user_agent_module(ctx):
  """Generate user agent specific GWT module."""
  if not ctx.attr.user_agent:
    return None

  ua = ctx.attr.user_agent
  impl = ua
  if ua in ALIASES:
    impl = ALIASES[ua]

  # intermediate artifact: user agent speific GWT xml file
  gwt_user_agent_xml = ctx.new_file(ctx.label.name + "_gwt.xml")
  ctx.file_action(output = gwt_user_agent_xml,
                  content=USER_AGENT_XML % (MODULE, impl))

  # intermediate artifact: user agent specific zip with GWT module
  gwt_user_agent_zip = ctx.new_file(ctx.label.name + "_gwt.zip")
  gwt = '%s_%s.gwt.xml' % (MODULE.replace('.', '/'), ua)
  dir = gwt_user_agent_zip.path + ".dir"
  cmd = " && ".join([
    "p=$PWD",
    "mkdir -p %s" % dir,
    "cd %s" % dir,
    "mkdir -p $(dirname %s)" % gwt,
    "cp $p/%s %s" % (gwt_user_agent_xml.path, gwt),
    "$p/%s cC $p/%s $(find . | sed 's|^./||')" % (ctx.executable._zip.path, gwt_user_agent_zip.path)
  ])
  ctx.action(
    inputs = [gwt_user_agent_xml] + ctx.files._zip,
    outputs = [gwt_user_agent_zip],
    command = cmd,
    mnemonic = "GenerateUserAgentGWTModule")

  return struct(
    zip=gwt_user_agent_zip,
    module=MODULE + '_' + ua
  )

def _gwt_binary_impl(ctx):
  module = ctx.attr.module[0]
  output_zip = ctx.outputs.output
  output_dir = output_zip.path + '.gwt_output'
  deploy_dir = output_zip.path + '.gwt_deploy'

  deps = _get_transitive_closure(ctx)

  paths = []
  for dep in deps:
    paths.append(dep.path)

  gwt_user_agent_modules = []
  ua = _gwt_user_agent_module(ctx)
  if ua:
    paths.append(ua.zip.path)
    gwt_user_agent_modules.append(ua.zip)
    module = ua.module

  cmd = "external/local_jdk/bin/java %s -Dgwt.normalizeTimestamps=true -cp %s %s -war %s -deploy %s " % (
    " ".join(ctx.attr.jvm_args),
    ":".join(paths),
    GWT_COMPILER,
    output_dir,
    deploy_dir,
  )
  # TODO(davido): clean up command concatenation
  cmd += " ".join([
    "-style %s" % ctx.attr.style,
    "-optimize %s" % ctx.attr.optimize,
    "-strict",
    " ".join(ctx.attr.compiler_args),
    module + "\n",
    "rm -rf %s/gwt-unitCache\n" % output_dir,
    "root=`pwd`\n",
    "cd %s; $root/%s Cc ../%s $(find .)\n" % (
      output_dir,
      ctx.executable._zip.path,
      output_zip.basename,
    )
  ])

  ctx.action(
    inputs = list(deps) + ctx.files._jdk + ctx.files._zip + gwt_user_agent_modules,
    outputs = [output_zip],
    mnemonic = "GwtBinary",
    progress_message = "GWT compiling " + output_zip.short_path,
    command = "set -e\n" + cmd,
  )

def _get_transitive_closure(ctx):
  deps = set_which_is_banned()
  for dep in ctx.attr.module_deps:
    deps += dep.java.transitive_runtime_deps
    deps += dep.java.transitive_source_jars
  for dep in ctx.attr.deps:
    if hasattr(dep, 'java'):
      deps += dep.java.transitive_runtime_deps
    elif hasattr(dep, 'files'):
      deps += dep.files

  return deps

gwt_binary = rule(
    attrs = {
        "user_agent": attr.string(),
        "style": attr.string(default = "OBF"),
        "optimize": attr.string(default = "9"),
        "deps": attr.label_list(allow_files = jar_filetype),
        "module": attr.string_list(default = [MODULE]),
        "module_deps": attr.label_list(allow_files = jar_filetype),
        "compiler_args": attr.string_list(),
        "jvm_args": attr.string_list(),
        "_jdk": attr.label(
            default = Label("//tools/defaults:jdk"),
        ),
        "_zip": attr.label(
            default = Label("@bazel_tools//tools/zip:zipper"),
            cfg = "host",
            executable = True,
            single_file = True,
        ),
    },
    outputs = {
        "output": "%{name}.zip",
    },
    implementation = _gwt_binary_impl,
)

def gwt_genrule(suffix = ""):
  dbg = 'ui_dbg' + suffix
  opt = 'ui_opt' + suffix
  module_dep = ':ui_module' + suffix
  args = GWT_COMPILER_ARGS_RELEASE_MODE if suffix == "_r" else GWT_COMPILER_ARGS

  genrule2(
    name = 'ui_optdbg' + suffix,
    srcs = [
      ':' + dbg,
      ':' + opt,
     ],
    cmd = 'cd $$TMP;' +
      'unzip -q $$ROOT/$(location :%s);' % dbg +
      'mv' +
      ' gerrit_ui/gerrit_ui.nocache.js' +
      ' gerrit_ui/dbg_gerrit_ui.nocache.js;' +
      'unzip -qo $$ROOT/$(location :%s);' % opt +
      'mkdir -p $$(dirname $@);' +
      'zip -qrD $$ROOT/$@ .',
    outs = ['ui_optdbg' + suffix + '.zip'],
    visibility = ['//visibility:public'],
   )

  gwt_binary(
    name = opt,
    module = [MODULE],
    module_deps = [module_dep],
    deps = DEPS,
    compiler_args = args,
    jvm_args = GWT_JVM_ARGS,
  )

  gwt_binary(
    name = dbg,
    style = 'PRETTY',
    optimize = "0",
    module_deps = [module_dep],
    deps = DEPS,
    compiler_args = GWT_COMPILER_ARGS,
    jvm_args = GWT_JVM_ARGS,
  )

def gen_ui_module(name, suffix = ""):
  gwt_module(
    name = name + suffix,
    srcs = native.glob(['src/main/java/**/*.java']),
    gwt_xml = 'src/main/java/%s.gwt.xml' % MODULE.replace('.', '/'),
    resources = native.glob(
        ['src/main/java/**/*'],
        exclude = ['src/main/java/**/*.java'] +
        ['src/main/java/%s.gwt.xml' % MODULE.replace('.', '/')]),
    deps = [
      '//gerrit-gwtui-common:diffy_logo',
      '//gerrit-gwtui-common:client',
      '//gerrit-gwtexpui:CSS',
      '//lib/codemirror:codemirror' + suffix,
      '//lib/gwt:user',
    ],
    visibility = ['//visibility:public'],
  )

def gwt_user_agent_permutations():
  for ua in BROWSERS:
    gwt_binary(
      name = "ui_%s" % ua,
      user_agent = ua,
      style = 'PRETTY',
      optimize = "0",
      module = [MODULE],
      module_deps = [':ui_module'],
      deps = DEPS,
      compiler_args = GWT_COMPILER_ARGS,
      jvm_args = GWT_JVM_ARGS,
    )
