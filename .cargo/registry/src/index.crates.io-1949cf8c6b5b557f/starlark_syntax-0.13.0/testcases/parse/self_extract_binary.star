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
"""Self-extracting binary.

Generate a binary suitable for self-extraction:

self_extract_binary(
  name = "install.sh",
  launcher = "launcher.sh",
  resources = ["path1/file1", "path2/file2"],
  flatten_resources = ["path3/file3"],
)

will generate a file 'install.sh' with a header (launcher.sh)
and a ZIP footer with the following entries:
  path1/
  path1/file1
  path2/
  path2/file2
  file3

"""

def _self_extract_binary(ctx):
  """Implementation for the self_extract_binary rule."""
  # This is a bit complex for stripping out timestamps
  zip_artifact = ctx.new_file(ctx.label.name + ".zip")
  touch_empty_files = [
      "mkdir -p $(dirname ${tmpdir}/%s); touch ${tmpdir}/%s" % (f, f)
      for f in ctx.attr.empty_files
      ]
  cp_resources = [
      ("mkdir -p $(dirname ${tmpdir}/%s)\n" % r.short_path +
       "cp %s ${tmpdir}/%s" % (r.path, r.short_path))
      for r in ctx.files.resources
      ]
  cp_flatten_resources = [
      "cp %s ${tmpdir}/%s" % (r.path, r.basename)
      for r in ctx.files.flatten_resources
      ]
  ctx.action(
      inputs = ctx.files.resources + ctx.files.flatten_resources,
      outputs = [zip_artifact],
      command = "\n".join([
          "tmpdir=$(mktemp -d ${TMPDIR:-/tmp}/tmp.XXXXXXXX)",
          "trap \"rm -fr ${tmpdir}\" EXIT"
          ] + touch_empty_files + cp_resources + cp_flatten_resources + [
              "find ${tmpdir} -exec touch -t 198001010000.00 '{}' ';'",
              "(d=${PWD}; cd ${tmpdir}; zip -rq ${d}/%s *)" % zip_artifact.path,
              ]),
      mnemonic = "ZipBin",
  )
  ctx.action(
      inputs = [ctx.file.launcher, zip_artifact],
      outputs = [ctx.outputs.executable],
      command = "\n".join([
          "cat %s %s > %s" % (ctx.file.launcher.path,
                              zip_artifact.path,
                              ctx.outputs.executable.path),
          "zip -qA %s" % ctx.outputs.executable.path
      ]),
      mnemonic = "BuildSelfExtractable",
  )

self_extract_binary = rule(
    _self_extract_binary,
    attrs = {
        "launcher": attr.label(
            mandatory = True,
            allow_files = True,
            single_file = True,
        ),
        "empty_files": attr.string_list(default = []),
        "resources": attr.label_list(
            default = [],
            allow_files = True,
        ),
        "flatten_resources": attr.label_list(
            default = [],
            allow_files = True,
        ),
    },
    executable = True,
)
