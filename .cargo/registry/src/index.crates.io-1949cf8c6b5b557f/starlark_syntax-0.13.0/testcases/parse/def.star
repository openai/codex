# @generated
# Copyright (C) 2017 The Bazel Authors.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
load("@io_bazel_rules_groovy//groovy:groovy.bzl", "groovy_library")

def bazel_groovy_test(name=None, test_class=None, size="small", deps=[], srcs=[], **kwargs):
    """A wrapper around groovy test to use the Bazel test runner."""
    test_class = "build.bazel.ci." + name if test_class == None else test_class
    groovy_library(
        name = name + "-lib",
        deps = deps,
        srcs = srcs)
    native.java_test(
        name = name,
        test_class = test_class,
        size = size,
        runtime_deps = deps + [
            "@org_codehaus_groovy_groovy_all//jar",
            "@org_hamcrest_hamcrest_all//jar",
            name + "-lib"],
        **kwargs)

def jenkins_library_test(deps=[], data=[], **kwargs):
    bazel_groovy_test(
        deps=deps+[
            "//jenkins/lib/tests/build/bazel/ci/utils",
            "//jenkins/lib/src/build/bazel/ci:lib"],
        data=data+["//jenkins/lib:lib-files"],
        **kwargs)
