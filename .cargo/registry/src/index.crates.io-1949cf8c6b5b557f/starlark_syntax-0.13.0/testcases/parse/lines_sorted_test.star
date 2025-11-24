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

load("@io_bazel_rules_go//go/private:tools/files_equal_test.bzl", "files_equal_test")

def lines_sorted_test(name, file, cmd="cat $< >$@", visibility=None, **kwargs):
  """Tests that lines within a file are sorted."""

  native.genrule(
      name = name + "_lines",
      testonly = True,
      srcs = [file],
      outs = [name + "_lines.txt"],
      cmd = cmd,
      visibility = visibility,
   )

  native.genrule(
      name = name + "_lines_sorted",
      testonly = True,
      srcs = [name + "_lines.txt"],
      outs = [name + "_lines_sorted.txt"],
      cmd = "sort $< >$@",
      visibility = visibility,
   )

  files_equal_test(
      name = name,
      actual = name + "_lines.txt",
      golden = name + "_lines_sorted.txt",
      visibility = visibility,
      **kwargs
   )
