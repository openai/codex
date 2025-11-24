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
"""Redirects for git repository rules."""

load(
    ":git.bzl",
    original_git_repository = "git_repository",
    original_new_git_repository = "new_git_repository",
)

def git_repository(**kwargs):
  print("The git_repository rule has been moved. Please load " +
        "@bazel_tools//tools/build_defs/repo:git.bzl instead. This redirect " +
        "will be removed in the future.")
  original_git_repository(**kwargs)

def new_git_repository(**kwargs):
  print("The new_git_repository rule has been moved. Please load " +
        "@bazel_tools//tools/build_defs/repo:git.bzl instead. This redirect " +
        "will be removed in the future.")
  original_new_git_repository(**kwargs)
