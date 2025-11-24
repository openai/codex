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

load("@io_bazel_rules_go//go/private:common.bzl",
    "split_srcs",
    "to_set",
    "sets",
)
load("@io_bazel_rules_go//go/private:mode.bzl",
    "get_mode",
    "mode_string",
)
load("@io_bazel_rules_go//go/private:providers.bzl",
    "GoLibrary",
    "GoSourceList",
    "GoArchive",
    "GoArchiveData",
    "sources",
)
load("@io_bazel_rules_go//go/platform:list.bzl",
    "GOOS",
    "GOARCH",
)

GoAspectProviders = provider()

def get_archive(dep):
  if GoAspectProviders in dep:
    return dep[GoAspectProviders].archive
  return dep[GoArchive]

def get_source_list(dep):
  if GoAspectProviders in dep:
    return dep[GoAspectProviders].source
  return dep[GoSourceList]


def collect_src(ctx, aspect=False, srcs = None, deps=None, want_coverage = None):
  rule = ctx.rule if aspect else ctx
  if srcs == None:
    srcs = rule.files.srcs
  if deps == None:
    deps = rule.attr.deps
  if want_coverage == None:
    want_coverage = ctx.coverage_instrumented() and not rule.label.name.endswith("~library~")
  return sources.merge([get_source_list(s) for s in rule.attr.embed] + [sources.new(
      srcs = srcs,
      deps = deps,
      gc_goopts = rule.attr.gc_goopts,
      runfiles = ctx.runfiles(collect_data = True),
      want_coverage = want_coverage,
  )])

def _go_archive_aspect_impl(target, ctx):
  mode = get_mode(ctx, ctx.rule.attr._go_toolchain_flags)
  if GoArchive not in target:
    if GoSourceList in target and hasattr(ctx.rule.attr, "embed"):
      return [GoAspectProviders(
        source = collect_src(ctx, aspect=True),
      )]
    return []
  goarchive = target[GoArchive]
  if goarchive.mode == mode:
    return [GoAspectProviders(
        source = target[GoSourceList],
        archive = goarchive,
    )]

  source = collect_src(ctx, aspect=True)
  for dep in ctx.rule.attr.deps:
    a = get_archive(dep)
    if a.mode != mode: fail("In aspect on {} found {} is {} expected {}".format(ctx.label, a.data.importpath, mode_string(a.mode), mode_string(mode)))

  go_toolchain = ctx.toolchains["@io_bazel_rules_go//go:toolchain"]
  goarchive = go_toolchain.actions.archive(ctx,
      go_toolchain = go_toolchain,
      mode = mode,
      importpath = target[GoLibrary].package.importpath,
      source = source,
      importable = True,
  )
  return [GoAspectProviders(
    source = source,
    archive = goarchive,
  )]

go_archive_aspect = aspect(
    _go_archive_aspect_impl,
    attr_aspects = ["deps", "embed"],
    attrs = {
        "pure": attr.string(values=["on", "off", "auto"]),
        "static": attr.string(values=["on", "off", "auto"]),
        "msan": attr.string(values=["on", "off", "auto"]),
        "race": attr.string(values=["on", "off", "auto"]),
        "goos": attr.string(values=GOOS.keys() + ["auto"], default="auto"),
        "goarch": attr.string(values=GOARCH.keys() + ["auto"], default="auto"),
    },
    toolchains = ["@io_bazel_rules_go//go:toolchain"],
)
