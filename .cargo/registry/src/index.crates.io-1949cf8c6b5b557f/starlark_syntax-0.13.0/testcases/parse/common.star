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

load("@io_bazel_rules_go//go/private:providers.bzl", "GoLibrary")
load("//go/private:skylib/lib/dicts.bzl", "dicts")
load("//go/private:skylib/lib/paths.bzl", "paths")
load("//go/private:skylib/lib/sets.bzl", "sets")
load("//go/private:skylib/lib/shell.bzl", "shell")
load("//go/private:skylib/lib/structs.bzl", "structs")
load("@io_bazel_rules_go//go/private:mode.bzl", "mode_string")

DEFAULT_LIB = "go_default_library"
VENDOR_PREFIX = "/vendor/"

go_exts = [
    ".go",
]

asm_exts = [
    ".s",
    ".S",
    ".h",  # may be included by .s
]

# be consistent to cc_library.
hdr_exts = [
    ".h",
    ".hh",
    ".hpp",
    ".hxx",
    ".inc",
]

c_exts = [
    ".c",
    ".cc",
    ".cxx",
    ".cpp",
    ".h",
    ".hh",
    ".hpp",
    ".hxx",
]

go_filetype = FileType(go_exts + asm_exts)
cc_hdr_filetype = FileType(hdr_exts)

# Extensions of files we can build with the Go compiler or with cc_library.
# This is a subset of the extensions recognized by go/build.
cgo_filetype = FileType(go_exts + asm_exts + c_exts)

def pkg_dir(workspace_root, package_name):
  """Returns a relative path to a package directory from the root of the
  sandbox. Useful at execution-time or run-time."""
  if workspace_root and package_name:
    return workspace_root + "/" + package_name
  if workspace_root:
    return workspace_root
  if package_name:
    return package_name
  return "."

def split_srcs(srcs):
  go = []
  headers = []
  asm = []
  c = []
  for src in srcs:
    if any([src.basename.endswith(ext) for ext in go_exts]):
      go.append(src)
    elif any([src.basename.endswith(ext) for ext in hdr_exts]):
      headers.append(src)
    elif any([src.basename.endswith(ext) for ext in asm_exts]):
      asm.append(src)
    elif any([src.basename.endswith(ext) for ext in c_exts]):
      c.append(src)
    else:
      fail("Unknown source type {0}".format(src.basename))
  return struct(
      go = go,
      headers = headers,
      asm = asm,
      c = c,
  )

def join_srcs(source):
  return source.go + source.headers + source.asm + source.c


def go_importpath(ctx):
  """Returns the expected importpath of the go_library being built.

  Args:
    ctx: The skylark Context

  Returns:
    Go importpath of the library
  """
  path = ctx.attr.importpath
  if path != "":
    return path
  path = ctx.attr._go_prefix.go_prefix
  if path.endswith("/"):
    path = path[:-1]
  if ctx.label.package:
    path += "/" + ctx.label.package
  if ctx.label.name != DEFAULT_LIB and not path.endswith(ctx.label.name):
    path += "/" + ctx.label.name
  if path.rfind(VENDOR_PREFIX) != -1:
    path = path[len(VENDOR_PREFIX) + path.rfind(VENDOR_PREFIX):]
  if path[0] == "/":
    path = path[1:]
  return path

def env_execute(ctx, arguments, environment = {}, **kwargs):
  """env_executes a command in a repository context. It prepends "env -i"
  to "arguments" before calling "ctx.execute".

  Variables that aren't explicitly mentioned in "environment"
  are removed from the environment. This should be preferred to "ctx.execute"
  in most situations.
  """
  env_args = ["env", "-i"]
  environment = dict(environment)
  for var in ["TMP", "TMPDIR"]:
    if var in ctx.os.environ and not var in environment:
      environment[var] = ctx.os.environ[var]
  for k, v in environment.items():
    env_args.append("%s=%s" % (k, v))
  return ctx.execute(env_args + arguments, **kwargs)

def to_set(v):
  if type(v) == "depset":
    fail("Do not pass a depset to to_set")
  return depset(v)

def declare_file(ctx, path="", ext="", mode=None, name = ""):
  filename = ""
  if mode:
    filename += mode_string(mode) + "/"
  filename += name if name else ctx.label.name
  if path:
    filename += "~/" + path
  if ext:
    filename += ext
  return ctx.actions.declare_file(filename)
