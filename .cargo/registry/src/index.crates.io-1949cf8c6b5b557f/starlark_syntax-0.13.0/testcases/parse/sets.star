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

"""Skylib module containing common set algorithms.

CAUTION: Operating on sets, particularly sets contained in providers, may
asymptotically slow down the analysis phase. While constructing large sets with
addition/union is fast (there is no linear-time copy involved), the
`difference` function and various comparison predicates involve linear-time
traversals.

For convenience, the functions in this module can take either sets or lists as
inputs; operations that take lists treat them as if they were sets (i.e.,
duplicate elements are ignored). Functions that return new sets always return
them as the `set` type, regardless of the types of the inputs.
"""


def _precondition_only_sets_or_lists(*args):
  """Verifies that all arguments are either sets or lists.

  The build will fail if any of the arguments is neither a set nor a list.

  Args:
    *args: A list of values that must be sets or lists.
  """
  for a in args:
    t = type(a)
    if t not in("depset", "list"):
      fail("Expected arguments to be depset or list, but found type %s: %r" %
           (t, a))


def _is_equal(a, b):
  """Returns whether two sets are equal.

  Args:
    a: A depset or a list.
    b: A depset or a list.
  Returns:
    True if `a` is equal to `b`, False otherwise.
  """
  _precondition_only_sets_or_lists(a, b)
  return sorted(depset(a)) == sorted(depset(b))


def _is_subset(a, b):
  """Returns whether `a` is a subset of `b`.

  Args:
    a: A depset or a list.
    b: A depset or a list.

  Returns:
    True if `a` is a subset of `b`, False otherwise.
  """
  _precondition_only_sets_or_lists(a, b)
  for e in a:
    if e not in b:
      return False
  return True


def _disjoint(a, b):
  """Returns whether two sets are disjoint.

  Two sets are disjoint if they have no elements in common.

  Args:
    a: A set or list.
    b: A set or list.

  Returns:
    True if `a` and `b` are disjoint, False otherwise.
  """
  _precondition_only_sets_or_lists(a, b)
  for e in a:
    if e in b:
      return False
  return True


def _intersection(a, b):
  """Returns the intersection of two sets.

  Args:
    a: A set or list.
    b: A set or list.

  Returns:
    A set containing the elements that are in both `a` and `b`.
  """
  _precondition_only_sets_or_lists(a, b)
  return depset([e for e in a if e in b])


def _union(*args):
  """Returns the union of several sets.

  Args:
    *args: An arbitrary number of sets or lists.

  Returns:
    The set union of all sets or lists in `*args`.
  """
  _precondition_only_sets_or_lists(*args)
  r = depset()
  for a in args:
    r += a
  return r


def _difference(a, b):
  """Returns the elements in `a` that are not in `b`.

  Args:
    a: A set or list.
    b: A set or list.

  Returns:
    A set containing the elements that are in `a` but not in `b`.
  """
  _precondition_only_sets_or_lists(a, b)
  return depset([e for e in a if e not in b])


sets = struct(
    difference = _difference,
    disjoint = _disjoint,
    intersection = _intersection,
    is_equal = _is_equal,
    is_subset = _is_subset,
    union = _union,
)
