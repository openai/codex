# @generated
# Copyright 2017 Google Inc. All rights reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
"""A rule for creating a D container image.

The signature of this rule is compatible with d_binary.
"""

load(
    "//lang:image.bzl",
    "dep_layer",
    "app_layer",
)
load(
    "//cc:image.bzl",
    "DEFAULT_BASE",
    _repositories = "repositories",
)
load("@io_bazel_rules_d//d:d.bzl", "d_binary")

def repositories():
  _repositories()

def d_image(name, base=None, deps=[], layers=[], **kwargs):
  """Constructs a container image wrapping a d_binary target.

  Args:
    layers: Augments "deps" with dependencies that should be put into
           their own layers.
    **kwargs: See d_binary.
  """
  binary_name = name + "_binary"

  if layers:
    print("d_image does not benefit from layers=[], got: %s" % layers)

  d_binary(name=binary_name, deps=deps + layers, **kwargs)

  index = 0
  base = base or DEFAULT_BASE
  for dep in layers:
    this_name = "%s_%d" % (name, index)
    dep_layer(name=this_name, base=base, dep=dep)
    base = this_name
    index += 1

  app_layer(name=name, base=base, binary=binary_name, layers=layers)
