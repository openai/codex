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
"""Common definitions for Apple rules."""

APPLE_SIMULATOR_ARCHITECTURES = ["i386", "x86_64"]
"""Architectures that are used by the simulator (iOS, tvOS and watchOS)."""

IOS_DEVICE_ARCHITECTURES = ["armv7", "arm64"]
"""Architectures that are used by iOS devices."""

TVOS_DEVICE_ARCHITECTURES = ["arm64"]
"""Architectures that are used by tvOS devices."""

WATCHOS_DEVICE_ARCHITECTURES = ["armv7k"]
"""Architectures that are used by watchOS devices."""

APPLE_DEFAULT_ARCHITECTURES = (APPLE_SIMULATOR_ARCHITECTURES +
                               IOS_DEVICE_ARCHITECTURES +
                               WATCHOS_DEVICE_ARCHITECTURES)
"""Architectures commonly used for building/testing on simulators/devices."""

APPLE_FRAGMENTS = ["apple"]
"""Configuration fragments containing Apple specific information."""

DARWIN_EXECUTION_REQUIREMENTS = {"requires-darwin": ""}
"""Standard execution requirements to force building on Mac.

See :func:`apple_action`."""

XCRUNWRAPPER_LABEL = "//external:xcrunwrapper"
"""The label for xcrunwrapper tool."""


def label_scoped_path(ctx, path):
  """Return the path scoped to target's label."""
  return ctx.label.name + "/" + path.lstrip("/")


def module_cache_path(ctx):
  """Returns the Clang module cache path to use for this rule."""
  return ctx.genfiles_dir.path + "/_objc_module_cache"


def apple_action(ctx, **kw):
  """Creates an action that only runs on MacOS/Darwin.

  Call it similar to how you would call ctx.action:
    apple_action(ctx, outputs=[...], inputs=[...],...)
  """
  execution_requirements = kw.get("execution_requirements", {})
  execution_requirements += DARWIN_EXECUTION_REQUIREMENTS

  no_sandbox = kw.pop("no_sandbox", False)
  if no_sandbox:
    execution_requirements["nosandbox"] = "1"

  kw["execution_requirements"] = execution_requirements

  ctx.action(**kw)


def xcrun_env(ctx):
  """Returns the environment dictionary necessary to use xcrunwrapper."""
  platform = ctx.fragments.apple.single_arch_platform

  if hasattr(apple_common, "apple_host_system_env"):
    xcode_config =  ctx.attr._xcode_config[apple_common.XcodeVersionConfig]
    return (apple_common.target_apple_env(xcode_config, platform) +
            apple_common.apple_host_system_env(xcode_config))
  else:
    return (ctx.fragments.apple.target_apple_env(platform) +
            ctx.fragments.apple.apple_host_system_env())


def xcrun_action(ctx, **kw):
  """Creates an apple action that executes xcrunwrapper.

  args:
    ctx: The context of the rule that owns this action.

  This method takes the same keyword arguments as ctx.action, however you don't
  need to specify the executable.
  """
  env = kw.get("env", {})
  kw["env"] = env + xcrun_env(ctx)

  apple_action(ctx, executable=ctx.executable._xcrunwrapper, **kw)
