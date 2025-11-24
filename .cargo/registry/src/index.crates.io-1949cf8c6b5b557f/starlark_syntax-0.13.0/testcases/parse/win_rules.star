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

# This is a quick and dirty rule to make Bazel compile itself.  It
# only supports Java.

def cc_library(srcs=[], hdrs=[], **kwargs):
  """Replace srcs and hdrs with a dummy.cc on non-Windows platforms."""
  native.cc_library(
      srcs = select({
            "//conditions:default": ["dummy.cc"],
            "//src/conditions:windows": srcs,
          }),
      hdrs = select({
            "//conditions:default": [],
            "//src/conditions:windows": hdrs,
          }),
      **kwargs)

def cc_binary(srcs=[], **kwargs):
  """Replace srcs with a dummy.cc on non-Windows platforms."""
  native.cc_binary(
      srcs = select({
            "//conditions:default": ["dummy.cc"],
            "//src/conditions:windows": srcs,
          }),
      **kwargs)

def cc_test(srcs=[], **kwargs):
  """Replace srcs with a dummy.cc on non-Windows platforms."""
  native.cc_test(
      srcs = select({
            "//conditions:default": ["dummy.cc"],
            "//src/conditions:windows": srcs,
          }),
      **kwargs)
