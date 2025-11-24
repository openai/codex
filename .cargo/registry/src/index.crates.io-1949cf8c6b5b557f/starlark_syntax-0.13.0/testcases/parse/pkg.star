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
"""Rules for manipulation of various packaging."""
load(":path.bzl", "dest_path", "compute_data_path")

# Filetype to restrict inputs
tar_filetype = [".tar", ".tar.gz", ".tgz", ".tar.xz", ".tar.bz2"]
deb_filetype = [".deb", ".udeb"]

def _pkg_tar_impl(ctx):
  """Implementation of the pkg_tar rule."""
  # Compute the relative path
  data_path = compute_data_path(ctx.outputs.out, ctx.attr.strip_prefix)

  build_tar = ctx.executable.build_tar
  args = [
      "--output=" + ctx.outputs.out.path,
      "--directory=" + ctx.attr.package_dir,
      "--mode=" + ctx.attr.mode,
      "--owner=" + ctx.attr.owner,
      "--owner_name=" + ctx.attr.ownername,
      ]
  file_inputs = ctx.files.srcs[:]
  args += ["--file=%s=%s" % (f.path, dest_path(f, data_path))
           for f in ctx.files.srcs]
  for target, f_dest_path in ctx.attr.files.items():
    target_files = target.files.to_list()
    if len(target_files) != 1:
      fail("Inputs to pkg_tar.files_map must describe exactly one file.")
    file_inputs += [target_files[0]]
    args += ["--file=%s=%s" % (target_files[0].path, f_dest_path)]
  if ctx.attr.modes:
    args += ["--modes=%s=%s" % (key, ctx.attr.modes[key]) for key in ctx.attr.modes]
  if ctx.attr.owners:
    args += ["--owners=%s=%s" % (key, ctx.attr.owners[key]) for key in ctx.attr.owners]
  if ctx.attr.ownernames:
    args += ["--owner_names=%s=%s" % (key, ctx.attr.ownernames[key])
             for key in ctx.attr.ownernames]
  if ctx.attr.extension:
    dotPos = ctx.attr.extension.find('.')
    if dotPos > 0:
      dotPos += 1
      args += ["--compression=%s" % ctx.attr.extension[dotPos:]]
  args += ["--tar=" + f.path for f in ctx.files.deps]
  args += ["--link=%s:%s" % (k, ctx.attr.symlinks[k])
           for k in ctx.attr.symlinks]
  arg_file = ctx.new_file(ctx.label.name + ".args")
  ctx.file_action(arg_file, "\n".join(args))

  ctx.action(
      command = "%s --flagfile=%s" % (build_tar.path, arg_file.path),
      inputs = file_inputs + ctx.files.deps + [arg_file, build_tar],
      outputs = [ctx.outputs.out],
      mnemonic="PackageTar",
      use_default_shell_env = True,
      )


def _pkg_deb_impl(ctx):
  """The implementation for the pkg_deb rule."""
  files = [ctx.file.data]
  args = [
      "--output=" + ctx.outputs.deb.path,
      "--changes=" + ctx.outputs.changes.path,
      "--data=" + ctx.file.data.path,
      "--package=" + ctx.attr.package,
      "--architecture=" + ctx.attr.architecture,
      "--maintainer=" + ctx.attr.maintainer,
      ]
  if ctx.attr.preinst:
    args += ["--preinst=@" + ctx.file.preinst.path]
    files += [ctx.file.preinst]
  if ctx.attr.postinst:
    args += ["--postinst=@" + ctx.file.postinst.path]
    files += [ctx.file.postinst]
  if ctx.attr.prerm:
    args += ["--prerm=@" + ctx.file.prerm.path]
    files += [ctx.file.prerm]
  if ctx.attr.postrm:
    args += ["--postrm=@" + ctx.file.postrm.path]
    files += [ctx.file.postrm]

  # Conffiles can be specified by a file or a string list
  if ctx.attr.conffiles_file:
    if ctx.attr.conffiles:
      fail("Both conffiles and conffiles_file attributes were specified")
    args += ["--conffile=@" + ctx.file.conffiles_file.path]
    files += [ctx.file.conffiles_file]
  elif ctx.attr.conffiles:
    args += ["--conffile=%s" % cf for cf in ctx.attr.conffiles]

  # Version and description can be specified by a file or inlined
  if ctx.attr.version_file:
    if ctx.attr.version:
      fail("Both version and version_file attributes were specified")
    args += ["--version=@" + ctx.file.version_file.path]
    files += [ctx.file.version_file]
  elif ctx.attr.version:
    args += ["--version=" + ctx.attr.version]
  else:
    fail("Neither version_file nor version attribute was specified")

  if ctx.attr.description_file:
    if ctx.attr.description:
      fail("Both description and description_file attributes were specified")
    args += ["--description=@" + ctx.file.description_file.path]
    files += [ctx.file.description_file]
  elif ctx.attr.description:
    args += ["--description=" + ctx.attr.description]
  else:
    fail("Neither description_file nor description attribute was specified")

  # Built using can also be specified by a file or inlined (but is not mandatory)
  if ctx.attr.built_using_file:
    if ctx.attr.built_using:
      fail("Both build_using and built_using_file attributes were specified")
    args += ["--built_using=@" + ctx.file.built_using_file.path]
    files += [ctx.file.built_using_file]
  elif ctx.attr.built_using:
    args += ["--built_using=" + ctx.attr.built_using]

  if ctx.attr.priority:
    args += ["--priority=" + ctx.attr.priority]
  if ctx.attr.section:
    args += ["--section=" + ctx.attr.section]
  if ctx.attr.homepage:
    args += ["--homepage=" + ctx.attr.homepage]

  args += ["--distribution=" + ctx.attr.distribution]
  args += ["--urgency=" + ctx.attr.urgency]
  args += ["--depends=" + d for d in ctx.attr.depends]
  args += ["--suggests=" + d for d in ctx.attr.suggests]
  args += ["--enhances=" + d for d in ctx.attr.enhances]
  args += ["--conflicts=" + d for d in ctx.attr.conflicts]
  args += ["--pre_depends=" + d for d in ctx.attr.predepends]
  args += ["--recommends=" + d for d in ctx.attr.recommends]

  ctx.action(
      executable = ctx.executable.make_deb,
      arguments = args,
      inputs = files,
      outputs = [ctx.outputs.deb, ctx.outputs.changes],
      mnemonic="MakeDeb"
      )
  ctx.action(
      command = "ln -s %s %s" % (ctx.outputs.deb.basename, ctx.outputs.out.path),
      inputs = [ctx.outputs.deb],
      outputs = [ctx.outputs.out])

# A rule for creating a tar file, see README.md
_real_pkg_tar = rule(
    implementation = _pkg_tar_impl,
    attrs = {
        "strip_prefix": attr.string(),
        "package_dir": attr.string(default="/"),
        "deps": attr.label_list(allow_files=tar_filetype),
        "srcs": attr.label_list(allow_files=True),
        "files": attr.label_keyed_string_dict(allow_files=True),
        "mode": attr.string(default="0555"),
        "modes": attr.string_dict(),
        "owner": attr.string(default="0.0"),
        "ownername": attr.string(default="."),
        "owners": attr.string_dict(),
        "ownernames": attr.string_dict(),
        "extension": attr.string(default="tar"),
        "symlinks": attr.string_dict(),
        # Implicit dependencies.
        "build_tar": attr.label(
            default=Label("//tools/build_defs/pkg:build_tar"),
            cfg="host",
            executable=True,
            allow_files=True)
    },
    outputs = {
        "out": "%{name}.%{extension}",
    },
    executable = False)

def pkg_tar(**kwargs):
  # Compatibility with older versions of pkg_tar that define files as
  # a flat list of labels.
  if "srcs" not in kwargs:
    if "files" in kwargs:
      if not hasattr(kwargs["files"], "items"):
        label = "%s//%s:%s" % (REPOSITORY_NAME, PACKAGE_NAME, kwargs["name"])
        print("%s: you provided a non dictionary to the pkg_tar `files` attribute. " % (label,) +
              "This attribute was renamed to `srcs`. " +
              "Consider renaming it in your BUILD file.")
        kwargs["srcs"] = kwargs.pop("files")
  _real_pkg_tar(**kwargs)

# A rule for creating a deb file, see README.md
pkg_deb = rule(
    implementation = _pkg_deb_impl,
    attrs = {
        "data": attr.label(mandatory=True, allow_files=tar_filetype, single_file=True),
        "package": attr.string(mandatory=True),
        "architecture": attr.string(default="all"),
        "distribution": attr.string(default="unstable"),
        "urgency": attr.string(default="medium"),
        "maintainer": attr.string(mandatory=True),
        "preinst": attr.label(allow_files=True, single_file=True),
        "postinst": attr.label(allow_files=True, single_file=True),
        "prerm": attr.label(allow_files=True, single_file=True),
        "postrm": attr.label(allow_files=True, single_file=True),
        "conffiles_file": attr.label(allow_files=True, single_file=True),
        "conffiles": attr.string_list(default=[]),
        "version_file": attr.label(allow_files=True, single_file=True),
        "version": attr.string(),
        "description_file": attr.label(allow_files=True, single_file=True),
        "description": attr.string(),
        "built_using_file": attr.label(allow_files=True, single_file=True),
        "built_using": attr.string(),
        "priority": attr.string(),
        "section": attr.string(),
        "homepage": attr.string(),
        "depends": attr.string_list(default=[]),
        "suggests": attr.string_list(default=[]),
        "enhances": attr.string_list(default=[]),
        "conflicts": attr.string_list(default=[]),
        "predepends": attr.string_list(default=[]),
        "recommends": attr.string_list(default=[]),
        # Implicit dependencies.
        "make_deb": attr.label(
            default=Label("//tools/build_defs/pkg:make_deb"),
            cfg="host",
            executable=True,
            allow_files=True)
    },
    outputs = {
        "out": "%{name}.deb",
        "deb": "%{package}_%{version}_%{architecture}.deb",
        "changes": "%{package}_%{version}_%{architecture}.changes"
    },
    executable = False)
