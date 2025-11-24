# @generated
# Copyright 2017 The Bazel Go Rules Authors. All rights reserved.
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

def _impl(ctx):
  ctx.actions.write(
      output = ctx.outputs.executable,
      content = "",
      is_executable = True,
  )

single_output_test = rule(
    implementation = _impl,
    attrs = {
        "dep": attr.label(allow_single_file = True),
    },
    test = True,
)
"""Checks that a dependency produces a single output file.

This test works by setting `allow_single_file = True` on the `dep` attribute.
If `dep` provides zero or multiple files in its `files` provider, Bazel will
fail to build this rule during analysis. The actual test does nothing.]

Args:
  dep: a label for the rule to check.
"""
