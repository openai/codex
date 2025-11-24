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
#
# Create a repository that produce the list of sources of Bazel in order to test
# that all sources in Bazel are contained in the //:srcs filegroup. CI systems
# can set the SRCS_EXCLUDES environment variable to exclude certain files from
# being considered as sources.
"""A repository definition to fetch all sources in Bazel."""

def _impl(rctx):
  workspace = rctx.path(Label("//:BUILD")).dirname
  srcs_excludes = "XXXXXXXXXXXXXX1268778dfsdf4"
  # Depending in ~/.git/logs/HEAD is a trick to depends on something that
  # change everytime the workspace content change.
  r = rctx.execute(["test", "-f", "%s/.git/logs/HEAD" % workspace])
  if r.return_code == 0:
    # We only add the dependency if it exists.
    unused_var = rctx.path(Label("//:.git/logs/HEAD"))  # pylint: disable=unused-variable

  if "SRCS_EXCLUDES" in rctx.os.environ:
    srcs_excludes = rctx.os.environ["SRCS_EXCLUDES"]
  r = rctx.execute(["find", str(workspace), "-type", "f"])
  rctx.file("find.result.raw", r.stdout.replace(str(workspace) + "/", ""))
  rctx.file("BUILD", """
genrule(
  name = "sources",
  outs = ["sources.txt"],
  srcs = ["find.result.raw"],
  visibility = ["//visibility:public"],
  cmd = " | ".join([
    "cat $<",
    "grep -Ev '^(\\\\.git|out/|output/|bazel-|derived|tools/defaults/BUILD)'",
    "grep -Ev '%s'",
    "sort -u > $@",
  ]),
)
""" % srcs_excludes)

list_source_repository = repository_rule(
    implementation = _impl,
    environ = ["SRCS_EXCLUDES"])
"""Create a //:sources target containing the list of sources of Bazel.

SRCS_EXCLUDES give a regex of files to excludes in the list."""
