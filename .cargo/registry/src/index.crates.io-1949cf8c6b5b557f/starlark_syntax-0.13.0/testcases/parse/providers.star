# @generated
# Copyright 2014 The Bazel Authors. All rights reserved.
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

load("@io_bazel_rules_go//go/private:mode.bzl", "mode_string")

GoLibrary = provider()
"""See go/providers.rst#GoLibrary for full documentation."""

GoPackage = provider()

GoPath = provider()

GoSource = provider()
"""See go/providers.rst#GoSource for full documentation."""

GoSourceList = provider()
"""See go/providers.rst#GoSourceList for full documentation."""

GoArchive = provider()
"""See go/providers.rst#GoArchive for full documentation."""

GoArchiveData = provider()

GoStdLib = provider()

def _merge_runfiles(a, b):
  if not a: return b
  if not b: return a
  return a.merge(b)

def _source_build_entry(srcs = [], deps = [], gc_goopts=[], runfiles=None, cgo_deps=[], cgo_exports=[], cgo_archive=None, want_coverage = False, source = None, exclude = None):
  """Creates a new GoSource from a collection of values and an optional GoSourceList to merge in."""
  for e in (source.entries if source else []):
    srcs = srcs + e.srcs
    deps = deps + e.deps
    gc_goopts = gc_goopts + e.gc_goopts
    runfiles = _merge_runfiles(runfiles, e.runfiles)
    cgo_deps = cgo_deps + e.cgo_deps
    cgo_exports = cgo_exports + e.cgo_exports
    if e.cgo_archive:
      if cgo_archive:
        fail("multiple libraries with cgo_archive embedded")
      cgo_archive = e.cgo_archive

  return GoSource(
      srcs = srcs,
      deps = deps,
      gc_goopts = gc_goopts,
      runfiles = runfiles,
      cgo_deps = cgo_deps,
      cgo_exports = cgo_exports,
      cgo_archive = cgo_archive,
      want_coverage = want_coverage,
      exclude = exclude,
  )

def _source_new(**kwargs):
  """Creates a new GoSourceList from a collection of values."""
  return GoSourceList(entries = [_source_build_entry(**kwargs)])

def _source_merge(source):
  """Merges the entries of multiple GoSourceList providers to a single GoSourceList."""
  entries = []
  for e in source:
    entries.extend(e.entries)
  return GoSourceList(entries = entries)

def _source_flatten(ctx, source):
  """Flattens a GoSourceList to a single GoSource ready for use."""
  return _source_build_entry(source = source)

def _source_filter(ctx, source, mode):
  return GoSourceList(entries = [s for s in source.entries if not (s.exclude and s.exclude(ctx, mode))])

sources = struct(
  new = _source_new,
  merge = _source_merge,
  flatten = _source_flatten,
  filter = _source_filter,
)
"""sources holds the functions for manipulating GoSourceList providers."""
