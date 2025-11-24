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

"""Skylib module containing file path manipulation functions.

NOTE: The functions in this module currently only support paths with Unix-style
path separators (forward slash, "/"); they do not handle Windows-style paths
with backslash separators or drive letters.
"""


def _basename(p):
  """Returns the basename (i.e., the file portion) of a path.

  Note that if `p` ends with a slash, this function returns an empty string.
  This matches the behavior of Python's `os.path.basename`, but differs from
  the Unix `basename` command (which would return the path segment preceding
  the final slash).

  Args:
    p: The path whose basename should be returned.
  Returns:
    The basename of the path, which includes the extension.
  """
  return p.rpartition("/")[-1]


def _dirname(p):
  """Returns the dirname of a path.

  The dirname is the portion of `p` up to but not including the file portion
  (i.e., the basename). Any slashes immediately preceding the basename are not
  included, unless omitting them would make the dirname empty.

  Args:
    p: The path whose dirname should be returned.
  Returns:
    The dirname of the path.
  """
  prefix, sep, _ = p.rpartition("/")
  if not prefix:
    return sep
  else:
    # If there are multiple consecutive slashes, strip them all out as Python's
    # os.path.dirname does.
    return prefix.rstrip("/")


def _is_absolute(path):
  """Returns `True` if `path` is an absolute path.

  Args:
    path: A path (which is a string).
  Returns:
    `True` if `path` is an absolute path.
  """
  return path.startswith("/")


def _join(path, *others):
  """Joins one or more path components intelligently.

  This function mimics the behavior of Python's `os.path.join` function on POSIX
  platform. It returns the concatenation of `path` and any members of `others`,
  inserting directory separators before each component except the first. The
  separator is not inserted if the path up until that point is either empty or
  already ends in a separator.

  If any component is an absolute path, all previous components are discarded.

  Args:
    path: A path segment.
    *others: Additional path segments.
  Returns:
    A string containing the joined paths.
  """
  result = path

  for p in others:
    if _is_absolute(p):
      result = p
    elif not result or result.endswith("/"):
      result += p
    else:
      result += "/" + p

  return result


def _normalize(path):
  """Normalizes a path, eliminating double slashes and other redundant segments.

  This function mimics the behavior of Python's `os.path.normpath` function on
  POSIX platforms; specifically:

  - If the entire path is empty, "." is returned.
  - All "." segments are removed, unless the path consists solely of a single
    "." segment.
  - Trailing slashes are removed, unless the path consists solely of slashes.
  - ".." segments are removed as long as there are corresponding segments
    earlier in the path to remove; otherwise, they are retained as leading ".."
    segments.
  - Single and double leading slashes are preserved, but three or more leading
    slashes are collapsed into a single leading slash.
  - Multiple adjacent internal slashes are collapsed into a single slash.

  Args:
    path: A path.
  Returns:
    The normalized path.
  """
  if not path:
    return "."

  if path.startswith("//") and not path.startswith("///"):
    initial_slashes = 2
  elif path.startswith("/"):
    initial_slashes = 1
  else:
    initial_slashes = 0
  is_relative = (initial_slashes == 0)

  components = path.split("/")
  new_components = []

  for component in components:
    if component in ("", "."):
      continue
    if component == "..":
      if new_components and new_components[-1] != "..":
        # Only pop the last segment if it isn't another "..".
        new_components.pop()
      elif is_relative:
        # Preserve leading ".." segments for relative paths.
        new_components.append(component)
    else:
      new_components.append(component)

  path = "/".join(new_components)
  if not is_relative:
    path = ("/" * initial_slashes) + path

  return path or "."


def _relativize(path, start):
  """Returns the portion of `path` that is relative to `start`.

  Because we do not have access to the underlying file system, this
  implementation differs slightly from Python's `os.path.relpath` in that it
  will fail if `path` is not beneath `start` (rather than use parent segments to
  walk up to the common file system root).

  Relativizing paths that start with parent directory references is not allowed.

  Args:
    path: The path to relativize.
    start: The ancestor path against which to relativize.
  Returns:
    The portion of `path` that is relative to `start`.
  """
  segments = _normalize(path).split("/")
  start_segments = _normalize(start).split("/")
  if start_segments == ["."]:
    start_segments = []
  start_length = len(start_segments)

  if (path.startswith("..") or start.startswith("..")):
    fail("Cannot relativize paths above the current (unknown) directory")

  if (path.startswith("/") != start.startswith("/") or
      len(segments) < start_length):
    fail("Path '%s' is not beneath '%s'" % (path, start))

  for ancestor_segment, segment in zip(start_segments, segments):
    if ancestor_segment != segment:
      fail("Path '%s' is not beneath '%s'" % (path, start))

  length = len(segments) - start_length
  result_segments = segments[-length:]
  return "/".join(result_segments)


def _replace_extension(p, new_extension):
  """Replaces the extension of the file at the end of a path.

  If the path has no extension, the new extension is added to it.

  Args:
    p: The path whose extension should be replaced.
    new_extension: The new extension for the file. The new extension should
        begin with a dot if you want the new filename to have one.
  Returns:
    The path with the extension replaced (or added, if it did not have one).
  """
  return _split_extension(p)[0] + new_extension


def _split_extension(p):
  """Splits the path `p` into a tuple containing the root and extension.

  Leading periods on the basename are ignored, so
  `path.split_extension(".bashrc")` returns `(".bashrc", "")`.

  Args:
    p: The path whose root and extension should be split.
  Returns:
    A tuple `(root, ext)` such that the root is the path without the file
    extension, and `ext` is the file extension (which, if non-empty, contains
    the leading dot). The returned tuple always satisfies the relationship
    `root + ext == p`.
  """
  b = _basename(p)
  last_dot_in_basename = b.rfind(".")

  # If there is no dot or the only dot in the basename is at the front, then
  # there is no extension.
  if last_dot_in_basename <= 0:
    return (p, "")

  dot_distance_from_end = len(b) - last_dot_in_basename
  return (p[:-dot_distance_from_end], p[-dot_distance_from_end:])


paths = struct(
    basename=_basename,
    dirname=_dirname,
    is_absolute=_is_absolute,
    join=_join,
    normalize=_normalize,
    relativize=_relativize,
    replace_extension=_replace_extension,
    split_extension=_split_extension,
)
