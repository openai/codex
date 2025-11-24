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

# Integration test framework for using Bazel
load("//tools:common.bzl", "BAZEL_VERSIONS")
load("//tools:repositories.bzl", "bazel_binary", "bazel_binaries")
load("//tools:bazel_java_integration_test.bzl", "bazel_java_integration_test", "bazel_java_integration_test_deps")
load("//tools:bazel_py_integration_test.bzl", "bazel_py_integration_test")
load("//go:bazel_integration_test.bzl", "bazel_go_integration_test")
