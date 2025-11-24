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
"""Rules for manipulation OCI images."""

load(
    "//container:container.bzl",
    "container_push",
    oci_bundle = "container_bundle",
    oci_flatten = "container_flatten",
    oci_image = "container_image",
    oci_import = "container_import",
    oci_pull = "container_pull",
)

def oci_push(*args, **kwargs):
  if "format" in kwargs:
    fail("Cannot override 'format' attribute on oci_push",
         attr="format")
  kwargs["format"] = "OCI"
  container_push(*args, **kwargs)
