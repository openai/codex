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

# Modes are documented in go/modes.rst#compilation-modes

LINKMODE_NORMAL = "normal"
LINKMODE_SHARED = "shared"
LINKMODE_PIE = "pie"
LINKMODE_PLUGIN = "plugin"

def mode_string(mode):
  result = [mode.goos, mode.goarch]
  if mode.static:
    result.append("static")
  if mode.race:
    result.append("race")
  if mode.msan:
    result.append("msan")
  if mode.pure:
    result.append("pure")
  if mode.debug:
    result.append("debug")
  if mode.strip:
    result.append("stripped")
  if not result or not mode.link == LINKMODE_NORMAL:
    result.append(mode.link)
  return "_".join(result)

def _ternary(*values):
  for v in values:
    if v == None: continue
    if type(v) == "bool": return v
    if type(v) != "string": fail("Invalid value type {}".format(type(v)))
    v = v.lower()
    if v == "on": return True
    if v == "off": return False
    if v == "auto": continue
    fail("Invalid value {}".format(v))
  fail("_ternary failed to produce a final result from {}".format(values))

def get_mode(ctx, toolchain_flags):
  if "@io_bazel_rules_go//go:toolchain" in ctx.toolchains:
    go_toolchain = ctx.toolchains["@io_bazel_rules_go//go:toolchain"]
  else:
    go_toolchain = ctx.toolchains["@io_bazel_rules_go//go:bootstrap_toolchain"]

  # We always have to use the pure stdlib in cross compilation mode
  force_pure = "on" if go_toolchain.cross_compile else "auto"

  #TODO: allow link mode selection
  static = _ternary(
      getattr(ctx.attr, "static", None),
      "static" in ctx.features,
  )
  race = _ternary(
      getattr(ctx.attr, "race", None),
      "race" in ctx.features,
  )
  msan = _ternary(
      getattr(ctx.attr, "msan", None),
      "msan" in ctx.features,
  )
  pure = _ternary(
      getattr(ctx.attr, "pure", None),
      force_pure,
      "pure" in ctx.features,
  )
  debug = ctx.var["COMPILATION_MODE"] == "debug"
  strip_mode = "sometimes"
  if toolchain_flags:
    strip_mode = toolchain_flags.strip
  strip = True
  if strip_mode == "always":
    strip = True
  elif strip_mode == "sometimes":
    strip = not debug
  goos = getattr(ctx.attr, "goos", None)
  if goos == None or goos == "auto":
    goos = go_toolchain.default_goos
  elif not pure:
    fail("If goos is set, pure must be true")
  goarch = getattr(ctx.attr, "goarch", None)
  if goarch == None or goarch == "auto":
    goarch = go_toolchain.default_goarch
  elif not pure:
    fail("If goarch is set, pure must be true")

  return struct(
      static = static,
      race = race,
      msan = msan,
      pure = pure,
      link = LINKMODE_NORMAL,
      debug = debug,
      strip = strip,
      goos = goos,
      goarch = goarch,
  )
