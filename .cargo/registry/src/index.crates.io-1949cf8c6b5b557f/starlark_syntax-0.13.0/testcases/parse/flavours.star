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
"""Create base images for docker tests."""
# TODO(dmarting): Right now we use a custom docker_pull that can build
# docker images, which is not reproducible and as a high cost, ideally
# we would switch to the docker_pull from bazelbuild/rules_docker but
# we do not have an easy mean to create and maintain the images we need
# for those tests.
load("//src/test/docker:docker_pull.bzl", "docker_pull")

FLAVOURS = [
    "centos6.7",
    "debian-stretch",
    "fedora23",
    "ubuntu-15.04",
    "ubuntu-16.04",
]

def pull_images_for_docker_tests():
  for flavour in FLAVOURS:
    docker_pull(
        name = "docker-" + flavour,
        tag = "bazel_tools_cpp_test:" + flavour,
        dockerfile = "//src/test/docker:Dockerfile." + flavour,
        optional = True,
    )
