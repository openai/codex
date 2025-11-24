# @generated
# Copyright 2017 The Bazel Authors. All rights reserved.
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

# Python integration test framework for using Bazel
load(":common.bzl", "BAZEL_VERSIONS")
load(":repositories.bzl", "bazel_binaries")

def bazel_py_integration_test(name, srcs, main=None, deps=[], versions=BAZEL_VERSIONS, **kwargs):
  """A wrapper around py_test that create several python tests, one per version
     of Bazel.

     Args:
       versions: list of version of bazel to create a test for. Each test
         will be named `<name>/bazel<version>`.
       See py_test for the other arguments.
  """
  if not main and len(srcs) == 1:
    main = srcs[0]
  for version in versions:
    add_deps = [
      str(Label("//bazel_integration_test:python")),
      str(Label("//bazel_integration_test:python_version_" + version)),
    ]
    native.py_test(
        name = "%s/bazel%s" % (name, version),
        srcs = srcs,
        main = main,
        deps = deps + add_deps,
        **kwargs)
  native.test_suite(
        name = name,
        tests = [":%s/bazel%s" % (name, version) for version in versions])
