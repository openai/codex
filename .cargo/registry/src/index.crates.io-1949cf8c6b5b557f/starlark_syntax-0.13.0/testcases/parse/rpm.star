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
"""Rules to create RPM archives."""

rpm_filetype = [".rpm"]
spec_filetype = [".spec"]

def _pkg_rpm_impl(ctx):
  """Implements to pkg_rpm rule."""

  files = []
  args = ["--name=" + ctx.label.name]

  # Version can be specified by a file or inlined
  if ctx.attr.version_file:
    if ctx.attr.version:
      fail("Both version and version_file attributes were specified")
    args += ["--version=@" + ctx.file.version_file.path]
    files += [ctx.file.version_file]
  elif ctx.attr.version:
    args += ["--version=" + ctx.attr.version]
  else:
    fail("Neither version_file nor version attribute was specified")

  if ctx.attr.architecture:
    args += ["--arch=" + ctx.attr.architecture]

  if ctx.attr.spec_file:
    args += ["--spec_file=" + ctx.file.spec_file.path]
    files += [ctx.file.spec_file]
  else:
    fail("spec_file was not specified")

  args += ["--out_file=" + ctx.outputs.rpm.path]

  # Add data files.
  files += [ctx.file.changelog] + ctx.files.data
  args += [ctx.file.changelog.path]
  for f in ctx.files.data:
    args += [f.path]

  # Call the generator script.
  # TODO(katre): Generate a source RPM.
  ctx.action(
      executable = ctx.executable._make_rpm,
      arguments = args,
      inputs = files,
      outputs = [ctx.outputs.rpm],
      mnemonic = "MakeRpm")

  # Link the RPM to the expected output name.
  ctx.action(
      command = "ln -s %s %s" % (ctx.outputs.rpm.basename, ctx.outputs.out.path),
      inputs = [ctx.outputs.rpm],
      outputs = [ctx.outputs.out])

# Define the rule.
pkg_rpm = rule(
    implementation = _pkg_rpm_impl,
    attrs = {
        "spec_file" : attr.label(mandatory=True, allow_files=spec_filetype, single_file=True),
        "architecture": attr.string(default="all"),
        "version_file": attr.label(allow_files=True, single_file=True),
        "version": attr.string(),
        "changelog" : attr.label(mandatory=True, allow_files=True, single_file=True),
        "data": attr.label_list(mandatory=True, allow_files=True),

        # Implicit dependencies.
        "_make_rpm": attr.label(
            default=Label("//tools/build_defs/pkg:make_rpm"),
            cfg="host",
            executable=True,
            allow_files=True),
    },
    outputs = {
        "out": "%{name}.rpm",
        "rpm": "%{name}-%{architecture}.rpm",
    },
    executable = False)
"""Creates an RPM format package from the data files.

This runs rpmbuild (and requires it to be installed beforehand) to generate
an RPM package based on the spec_file and data attributes.

Args:
  spec_file: The RPM spec file to use. If the version or version_file
    attributes are provided, the Version in the spec will be overwritten.
    Any Sources listed in the spec file must be provided as data dependencies.
  version: The version of the package to generate. This will overwrite any
    Version provided in the spec file. Only specify one of version and
    version_file.
  version_file: A file containing the version of the package to generate. This
    will overwrite any Version provided in the spec file. Only specify one of
    version and version_file.
  changelog: A changelog file to include. This will not be written to the spec
    file, which should only list changes to the packaging, not the software itself.
  data: List all files to be included in the package here.
"""
