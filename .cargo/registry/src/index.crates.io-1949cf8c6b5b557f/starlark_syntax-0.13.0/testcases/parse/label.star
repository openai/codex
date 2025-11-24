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
"""Rules for dealing with labels and their string form."""

def string_to_label(label_list, string_list):
  """Form a mapping from label strings to the resolved label."""
  label_string_dict = dict()
  for i in range(len(label_list)):
    string = string_list[i]
    label = label_list[i]
    label_string_dict[string] = label
  return label_string_dict
