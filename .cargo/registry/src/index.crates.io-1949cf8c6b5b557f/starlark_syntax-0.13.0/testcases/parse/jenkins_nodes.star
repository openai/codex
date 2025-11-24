# @generated
# Copyright 2016 The Bazel Authors. All rights reserved.
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

# Macros to ease the creation of machines
load(":jenkins_node.bzl", "jenkins_node")

def jenkins_node_names(name, count):
  """Returns the names for `count` production jenkins node prefixed by `name`."""
  return ["%s-%s" % (name, i) for i in range(1, count+1)]

def _extend_kwargs(kwargs, extra_args):
  result = {}
  for k,v in kwargs.items():
    result[k] = v
  if extra_args:
    for k,v in extra_args.items():
      result[k] = v
  return result

def jenkins_nodes(name,
                  count,
                  labels=None,
                  prod_args=None,
                  staging_args=None,
                  install_bazel=True,
                  **kwargs):
  """Create a set of Jenkins nodes on the system.

  It creates `count` production nodes with name prefix `name`.

  Example:
  If `name` is `darwin-x86_64` and `count` is two, it will
  create two production nodes `darwin-x86_64-1` and
  `darwin-x86_64-2` and one staging node `darwin-x86_64-staging`.

  Args:
    name: prefix of each node name, it should be the platform
      name (e.g., darwin-x86_64, ubuntu-14.04-x86_64, ...).
    count: number of production node to create.
    labels: Jenkins node labels to apply to this node (in addition to
      the "install-bazel" label and the `name` itself).
    prod_args: dictionary of additional arguments for production only
      nodes that will be passed to `jenkins_node`.
    staging_args: dictionary of additional arguments for staging only
      nodes that will be passed to `jenkins_node`.
    install_bazel: if the "install-bazel" label should be added to labels.
    **kwargs: other arguments to be passed verbatim to `jenkins_node`.
  """
  labels = [
      name] + (["install-bazel"] if install_bazel else []) + (labels if labels else [])
  prod_kwargs = _extend_kwargs(kwargs, prod_args)
  [jenkins_node(
      name = n,
      labels = labels,
      **prod_kwargs
  ) for n in jenkins_node_names(name, count)]
  staging_kwargs = _extend_kwargs(kwargs, staging_args)
  jenkins_node(
      name = "%s-staging" % name,
      labels = labels,
      **staging_kwargs)
