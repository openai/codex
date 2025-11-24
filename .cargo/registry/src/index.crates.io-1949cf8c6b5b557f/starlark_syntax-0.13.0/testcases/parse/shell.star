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

"""Skylib module containing shell utility functions."""


def _array_literal(iterable):
  """Creates a string from a sequence that can be used as a shell array.

  For example, `shell.array_literal(["a", "b", "c"])` would return the string
  `("a" "b" "c")`, which can be used in a shell script wherever an array
  literal is needed.

  Note that all elements in the array are quoted (using `shell.quote`) for
  safety, even if they do not need to be.

  Args:
    iterable: A sequence of elements. Elements that are not strings will be
        converted to strings first, by calling `str()`.
  Returns:
    A string that represents the sequence as a shell array; that is,
    parentheses containing the quoted elements.
  """
  return "(" + " ".join([_quote(str(i)) for i in iterable]) + ")"


def _quote(s):
  """Quotes the given string for use in a shell command.

  This function quotes the given string (in case it contains spaces or other
  shell metacharacters.)

  Args:
    s: The string to quote.
  Returns:
    A quoted version of the string that can be passed to a shell command.
  """
  return "'" + s.replace("'", "'\\''") + "'"


shell = struct(
    array_literal=_array_literal,
    quote=_quote,
)
