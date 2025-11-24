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

"""Rules that allows select() to differentiate between Apple OS versions."""

def _strip_version(version):
  """Strip trailing characters that aren't digits or '.' from version names.

  Some OS versions look like "9.0gm", which is not useful for select()
  statements. Thus, we strip the trailing "gm" part.

  Args:
    version: the version string

  Returns:
    The version with trailing letters stripped.
  """
  result = ""
  for ch in str(version):
    if not ch.isdigit() and ch != ".":
      break

    result += ch

  return result


def _xcode_version_flag_impl(ctx):
  """A rule that allows select() to differentiate between Xcode versions."""
  xcode_config = ctx.attr._xcode_config[apple_common.XcodeVersionConfig]
  return struct(providers = [
      config_common.FeatureFlagInfo(value = _strip_version(
          xcode_config.xcode_version()))])


def _ios_sdk_version_flag_impl(ctx):
  """A rule that allows select() to select based on the iOS SDK version."""
  xcode_config = ctx.attr._xcode_config[apple_common.XcodeVersionConfig]

  return struct(providers = [
      config_common.FeatureFlagInfo(value = _strip_version(
          xcode_config.sdk_version_for_platform(
              apple_common.platform.ios_device)))])


def _tvos_sdk_version_flag_impl(ctx):
  """A rule that allows select() to select based on the tvOS SDK version."""
  xcode_config = ctx.attr._xcode_config[apple_common.XcodeVersionConfig]

  return struct(providers = [
      config_common.FeatureFlagInfo(value = _strip_version(
          xcode_config.sdk_version_for_platform(
              apple_common.platform.tvos_device)))])


def _watchos_sdk_version_flag_impl(ctx):
  """A rule that allows select() to select based on the watchOS SDK version."""
  xcode_config = ctx.attr._xcode_config[apple_common.XcodeVersionConfig]

  return struct(providers = [
      config_common.FeatureFlagInfo(value = _strip_version(
          xcode_config.sdk_version_for_platform(
              apple_common.platform.watchos_device)))])


def _macos_sdk_version_flag_impl(ctx):
  """A rule that allows select() to select based on the macOS SDK version."""
  xcode_config = ctx.attr._xcode_config[apple_common.XcodeVersionConfig]

  return struct(providers = [
      config_common.FeatureFlagInfo(value = _strip_version(
          xcode_config.sdk_version_for_platform(
              apple_common.platform.macos)))])


xcode_version_flag = rule(
    implementation = _xcode_version_flag_impl,
    attrs = {
        "_xcode_config": attr.label(default=Label("//tools/osx:current_xcode_config")),
    })

ios_sdk_version_flag = rule(
    implementation = _ios_sdk_version_flag_impl,
    attrs = {
        "_xcode_config": attr.label(default=Label("//tools/osx:current_xcode_config")),
    })

tvos_sdk_version_flag = rule(
    implementation = _tvos_sdk_version_flag_impl,
    attrs = {
        "_xcode_config": attr.label(default=Label("//tools/osx:current_xcode_config")),
    })

watchos_sdk_version_flag = rule(
    implementation = _watchos_sdk_version_flag_impl,
    attrs = {
        "_xcode_config": attr.label(default=Label("//tools/osx:current_xcode_config")),
    })

macos_sdk_version_flag = rule(
    implementation = _macos_sdk_version_flag_impl,
    attrs = {
        "_xcode_config": attr.label(default=Label("//tools/osx:current_xcode_config")),
    })
