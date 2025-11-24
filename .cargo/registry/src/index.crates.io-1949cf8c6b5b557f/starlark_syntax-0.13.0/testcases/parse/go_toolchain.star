# @generated
# Copyright 2016 The Bazel Go Rules Authors. All rights reserved.
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
"""
Toolchain rules used by go.
"""
load("@io_bazel_rules_go//go/private:actions/archive.bzl", "emit_archive")
load("@io_bazel_rules_go//go/private:actions/asm.bzl", "emit_asm")
load("@io_bazel_rules_go//go/private:actions/binary.bzl", "emit_binary")
load("@io_bazel_rules_go//go/private:actions/compile.bzl", "emit_compile", "bootstrap_compile")
load("@io_bazel_rules_go//go/private:actions/cover.bzl", "emit_cover")
load("@io_bazel_rules_go//go/private:actions/library.bzl", "emit_library")
load("@io_bazel_rules_go//go/private:actions/link.bzl", "emit_link", "bootstrap_link")
load("@io_bazel_rules_go//go/private:actions/pack.bzl", "emit_pack")
load("@io_bazel_rules_go//go/private:providers.bzl", "GoStdLib")
load("@io_bazel_rules_go//go/platform:list.bzl", "GOOS_GOARCH")
load("@io_bazel_rules_go//go/private:mode.bzl", "mode_string")

def _get_stdlib(ctx, go_toolchain, mode):
  for stdlib in go_toolchain.stdlib.all:
    stdlib = stdlib[GoStdLib]
    if (stdlib.goos == mode.goos and
        stdlib.goarch == mode.goarch and
        stdlib.race == mode.race and
        stdlib.pure == mode.pure):
      return stdlib
  fail("No matching standard library for "+mode_string(mode))

def _goos_to_extension(goos):
  if goos == "windows":
    return ".exe"
  return ""

def _go_toolchain_impl(ctx):
  return [platform_common.ToolchainInfo(
      name = ctx.label.name,
      cross_compile = ctx.attr.cross_compile,
      default_goos = ctx.attr.goos,
      default_goarch = ctx.attr.goarch,
      stdlib = struct(
          all = ctx.attr._stdlib_all,
          get = _get_stdlib,
      ),
      actions = struct(
          archive = emit_archive,
          asm = emit_asm,
          binary = emit_binary,
          compile = emit_compile if ctx.executable._compile else bootstrap_compile,
          cover = emit_cover,
          library = emit_library,
          link = emit_link if ctx.executable._link else bootstrap_link,
          pack = emit_pack,
      ),
      tools = struct(
          asm = ctx.executable._asm,
          compile = ctx.executable._compile,
          pack = ctx.executable._pack,
          link = ctx.executable._link,
          cgo = ctx.executable._cgo,
          test_generator = ctx.executable._test_generator,
          cover = ctx.executable._cover,
      ),
      flags = struct(
          compile = (),
          link = ctx.attr.link_flags,
          link_cgo = ctx.attr.cgo_link_flags,
      ),
      data = struct(
          crosstool = ctx.files._crosstool,
          package_list = ctx.file._package_list,
          extension = _goos_to_extension(ctx.attr.goos),
      ),
  )]

def _stdlib_all():
  stdlibs = []
  for goos, goarch in GOOS_GOARCH:
    stdlibs.extend([
      Label("@go_stdlib_{}_{}_cgo".format(goos, goarch)),
      Label("@go_stdlib_{}_{}_pure".format(goos, goarch)),
      Label("@go_stdlib_{}_{}_cgo_race".format(goos, goarch)),
      Label("@go_stdlib_{}_{}_pure_race".format(goos, goarch)),
    ])
  return stdlibs

def _asm(bootstrap):
  if bootstrap:
    return None
  return Label("//go/tools/builders:asm")

def _compile(bootstrap):
  if bootstrap:
    return None
  return Label("//go/tools/builders:compile")

def _pack(bootstrap):
  if bootstrap:
    return None
  return Label("//go/tools/builders:pack")

def _link(bootstrap):
  if bootstrap:
    return None
  return Label("//go/tools/builders:link")

def _cgo(bootstrap):
  if bootstrap:
    return None
  return Label("//go/tools/builders:cgo")

def _test_generator(bootstrap):
  if bootstrap:
    return None
  return Label("//go/tools/builders:generate_test_main")

def _cover(bootstrap):
  if bootstrap:
    return None
  return Label("//go/tools/builders:cover")

_go_toolchain = rule(
    _go_toolchain_impl,
    attrs = {
        # Minimum requirements to specify a toolchain
        "goos": attr.string(mandatory = True),
        "goarch": attr.string(mandatory = True),
        "cross_compile": attr.bool(default = False),
        # Optional extras to a toolchain
        "link_flags": attr.string_list(default = []),
        "cgo_link_flags": attr.string_list(default = []),
        "bootstrap": attr.bool(default = False),
        # Tools, missing from bootstrap toolchains
        "_asm": attr.label(allow_files = True, single_file = True, executable = True, cfg = "host", default = _asm),
        "_compile": attr.label(allow_files = True, single_file = True, executable = True, cfg = "host", default = _compile),
        "_pack": attr.label(allow_files = True, single_file = True, executable = True, cfg = "host", default = _pack),
        "_link": attr.label(allow_files = True, single_file = True, executable = True, cfg = "host", default = _link),
        "_cgo": attr.label(allow_files = True, single_file = True, executable = True, cfg = "host", default = _cgo),
        "_test_generator": attr.label(allow_files = True, single_file = True, executable = True, cfg = "host", default = _test_generator),
        "_cover": attr.label(allow_files = True, single_file = True, executable = True, cfg = "host", default = _cover),
        # Hidden internal attributes
        "_stdlib_all": attr.label_list(default = _stdlib_all()),
        "_crosstool": attr.label(default=Label("//tools/defaults:crosstool")),
        "_package_list": attr.label(allow_files = True, single_file = True, default="@go_sdk//:packages.txt"),
    },
)

def go_toolchain(name, target, host=None, constraints=[], **kwargs):
  """See go/toolchains.rst#go-toolchain for full documentation."""

  if not host: host = target
  cross = host != target
  goos, _, goarch = target.partition("_")
  target_constraints = constraints + [
    "@io_bazel_rules_go//go/toolchain:" + goos,
    "@io_bazel_rules_go//go/toolchain:" + goarch,
  ]
  host_goos, _, host_goarch = host.partition("_")
  exec_constraints = [
      "@io_bazel_rules_go//go/toolchain:" + host_goos,
      "@io_bazel_rules_go//go/toolchain:" + host_goarch,
  ]

  impl_name = name + "-impl"
  _go_toolchain(
      name = impl_name,
      goos = goos,
      goarch = goarch,
      cross_compile = cross,
      bootstrap = False,
      tags = ["manual"],
      visibility = ["//visibility:public"],
      **kwargs
  )
  native.toolchain(
      name = name,
      toolchain_type = "@io_bazel_rules_go//go:toolchain",
      exec_compatible_with = exec_constraints,
      target_compatible_with = target_constraints,
      toolchain = ":"+impl_name,
  )

  if not cross:
    # If not cross, register a bootstrap toolchain
    name = name + "-bootstrap"
    impl_name = name + "-impl"
    _go_toolchain(
        name = impl_name,
        goos = goos,
        goarch = goarch,
        bootstrap = True,
        tags = ["manual"],
        visibility = ["//visibility:public"],
        **kwargs
    )
    native.toolchain(
        name = name,
        toolchain_type = "@io_bazel_rules_go//go:bootstrap_toolchain",
        exec_compatible_with = exec_constraints,
        target_compatible_with = target_constraints,
        toolchain = ":"+impl_name,
    )

def _go_toolchain_flags(ctx):
    return struct(
        strip = ctx.attr.strip,
    )

go_toolchain_flags = rule(
    _go_toolchain_flags,
    attrs = {
        "strip": attr.string(mandatory=True),
    },
)
