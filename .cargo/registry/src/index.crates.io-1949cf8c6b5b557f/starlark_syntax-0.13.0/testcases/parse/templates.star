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

# Rules for templating / files layout

def _expand_template_impl(ctx):
  """Simply spawn the template-engine in a rule."""
  variables = [
      "--variable=%s=%s" % (k, ctx.attr.substitutions[k])
      for k in ctx.attr.substitutions
  ]
  if not ctx.attr.escape_xml:
    variables += ["--noescape_xml"]
  d = {str(ctx.attr.deps[i].label): ctx.files.deps[i].path
       for i in range(0, len(ctx.attr.deps))}
  imports = ["--imports=%s=%s" % (k, d[k]) for k in d]
  imports += ["--imports=%s=%s" % (k, d[str(ctx.label.relative(ctx.attr.deps_aliases[k]))])
              for k in ctx.attr.deps_aliases]
  ctx.action(
      executable = ctx.executable._engine,
      arguments = [
        "--executable" if ctx.attr.executable else "--noexecutable",
        "--template=%s" % ctx.file.template.path,
        "--output=%s" % ctx.outputs.out.path,
      ] + variables + imports,
      inputs = [ctx.file.template] + ctx.files.deps,
      outputs = [ctx.outputs.out],
      )

expand_template = rule(
    attrs = {
        "template": attr.label(
            mandatory = True,
            allow_files = True,
            single_file = True,
        ),
        "deps": attr.label_list(default = [], allow_files = True),
        "deps_aliases": attr.string_dict(default = {}),
        "substitutions": attr.string_dict(mandatory = True),
        "out": attr.output(mandatory = True),
        "executable": attr.bool(default = True),
        "escape_xml": attr.bool(default = True),
        "_engine": attr.label(
            default = Label("//templating:template_engine"),
            executable = True,
            cfg="host"),
    },
    implementation = _expand_template_impl,
)
"""Expand a jinja2 template file.

This rules expands the file given in template, into the file given by out.

Args:
  template: The template file to expand.
  deps: additional files to expand, they will be accessible as imports[label]
      in the template environment. If a file ends with .tpl, it is considered
      a template itself and will be expanded.
  deps_aliases: a dictionary of name to label. Each label in that dictionary
      should be present in the deps attribute, and will be make accessible as
      imports[name] in the template environment.
  substitutions: a dictionary of key => values that will appear as variables.key
      in the template environment.
  out: the name of the output file to generate.
  executable: mark the result as executable if set to True.
"""

def strip_prefix(path, prefixes):
  for prefix in prefixes:
    if path.startswith(prefix):
      return path[len(prefix):]
  return path

def strip_suffix(path, suffixes):
  for suffix in suffixes:
    if path.endswith(suffix):
      return path[:-len(suffix)]
  return path

def _dest_path(f, strip_prefixes, strip_suffixes):
  """Returns the short path of f, stripped of strip_prefixes and strip_suffixes."""
  return strip_suffix(strip_prefix(f.short_path, strip_prefixes), strip_suffixes)

def _format_path(path_format, path):
  dirsep = path.rfind("/")
  dirname = path[:dirsep] if dirsep > 0 else ""
  basename = path[dirsep+1:] if dirsep > 0 else path
  extsep = basename.rfind(".")
  extension = basename[extsep+1:] if extsep > 0 else ""
  basename = basename[:extsep] if extsep > 0 else basename
  return path_format.format(
      path=path,
      dirname=dirname,
      basename=basename,
      extension=extension
  )

def _append_inputs(args, inputs, f, path, path_format):
  args.append("--file=%s=%s" % (
      f.path,
      _format_path(path_format, path)
  ))
  inputs.append(f)

def _merge_files_impl(ctx):
  """Merge a list of config files in a tar ball with the correct layout."""
  output = ctx.outputs.out
  build_tar = ctx.executable._build_tar
  inputs = []
  args = [
      "--output=" + output.path,
      "--directory=" + ctx.attr.directory,
      "--mode=0644",
      ]
  variables = [
      "--variable=%s=%s" % (k, ctx.attr.substitutions[k])
      for k in ctx.attr.substitutions
  ]
  for f in ctx.files.srcs:
    path = _dest_path(f, ctx.attr.strip_prefixes, ctx.attr.strip_suffixes)
    if path.endswith(ctx.attr.template_extension):
      path = path[:-4]
      f2 = ctx.new_file(ctx.label.name + "/" + path)
      ctx.action(
          executable = ctx.executable._engine,
          arguments = [
            "--template=%s" % f.path,
            "--output=%s" % f2.path,
            "--noescape_xml",
          ] + variables,
          inputs = [f],
          outputs = [f2],
      )
      _append_inputs(args, inputs, f2, path, ctx.attr.path_format)
    else:
      _append_inputs(args, inputs, f, path, ctx.attr.path_format)
  ctx.action(
      executable = build_tar,
      arguments = args,
      inputs = inputs,
      outputs = [output],
      mnemonic="MergeFiles"
      )

merge_files = rule(
    attrs = {
        "srcs": attr.label_list(allow_files=True),
        "template_extension": attr.string(default=".tpl"),
        "directory": attr.string(default="/"),
        "strip_prefixes": attr.string_list(default=[]),
        "strip_suffixes": attr.string_list(default=["-staging", "-test"]),
        "substitutions": attr.string_dict(default={}),
        "path_format": attr.string(default="{path}"),
        "_build_tar": attr.label(
            default=Label("@bazel_tools//tools/build_defs/pkg:build_tar"),
            cfg="host",
            executable=True,
            allow_files=True),
        "_engine": attr.label(
            cfg="host",
            default = Label("//templating:template_engine"),
            executable = True),
    },
    outputs = {"out": "%{name}.tar"},
    implementation = _merge_files_impl,
)
"""Merge a set of files in a single tarball.

This rule merge a set of files into one tarball, each file will appear in the
tarball as a file determined by path_format, strip_prefixes and directory.

Outputs:
  <name>.tar: the tarball containing all the files in srcs.

Args:
  srcs: The list of files to merge. If a file is ending with ".tpl" (see
      template_extension), it will get expanded like a template passed to
      expand_template.
  template_extension: extension of files to be considered as template, ".tpl"
      by default.
  directory: base directory for all the files in the resulting tarball.
  strip_prefixes: list of prefixes to strip from the path of the srcs to obtain
      the final path (see path_format).
  strip_suffixes: list of suffixes to strip from the path of the srcs to obtain
      the final path (see path_format).
  substitutions: map of substitutions to make available during template
      expansion. Values of that map will be available as "variables.name" in
      the template environment.
  path_format: format of the final files. Each file will appear in the final
      tarball under "{directory}/{path_format}" where the following string of
      path_format are replaced:
          {path}: path of the input file, removed from prefixes and suffixes,
          {dirname}: directory name of path,
          {basename}: base filename of path,
          {extension}: extension of path
"""
