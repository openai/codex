# @generated
# Copyright 2016 The Bazel Go Rules Authors. All rights reserved.
# Copyright 2016 The Closure Rules Authors. All rights reserved.
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

"""Tests that two files contain the same data."""

def _impl(ctx):
  if ctx.file.golden == ctx.file.actual:
    fail("GOLDEN and ACTUAL should be different files")
  ctx.actions.write(
      output=ctx.outputs.executable,
      content="\n".join([
          "#!/bin/bash",
          "function checksum() {",
          "  if command -v openssl >/dev/null; then",
          "    openssl sha1 $1 | cut -f 2 -d ' '",
          "  elif command -v sha256sum >/dev/null; then",
          "    sha256sum $1 | cut -f 1 -d ' '",
          "  elif command -v shasum >/dev/null; then",
          "    cat $1 | shasum -a 256 | cut -f 1 -d ' '",
          "  else",
          "    echo please install openssl >&2",
          "    exit 1",
          "  fi",
          "}",
          "SUM1=$(checksum %s)" % _runpath(ctx.file.golden),
          "SUM2=$(checksum %s)" % _runpath(ctx.file.actual),
          "if [[ ${SUM1} != ${SUM2} ]]; then",
          "  echo ERROR: %s >&2" % ctx.attr.error_message,
          "  echo %s ${SUM1} >&2" % _runpath(ctx.file.golden),
          "  echo %s ${SUM2} >&2" % _runpath(ctx.file.actual),
          "  exit 1",
          "fi",
      ]),
      is_executable=True)
  return struct(runfiles=ctx.runfiles([ctx.file.golden,
                                       ctx.file.actual]))

def _runpath(f):
  """Figures out the proper runfiles path for a file, using voodoo"""
  if f.path.startswith('bazel-out/'):
    return f.short_path
  else:
    return f.path

files_equal_test = rule(
    attrs = {
        "golden": attr.label(
            mandatory = True,
            allow_files = True,
            single_file = True),
        "actual": attr.label(
            mandatory = True,
            allow_files = True,
            single_file = True),
        "error_message": attr.string(
            default="FILES DO NOT HAVE EQUAL CONTENTS"),
    },
    implementation = _impl,
    test = True)
