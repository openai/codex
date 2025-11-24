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
"""Repository rule to generate host xcode_config and xcode_version targets.

   The xcode_config and xcode_version targets are configured for xcodes/SDKs
   installed on the local host.
"""


def _search_string(fullstring, prefix, suffix):
  """Returns the substring between two given substrings of a larger string.

  Args:
    fullstring: The larger string to search.
    prefix: The substring that should occur directly before the returned string.
    suffix: The substring that should occur directly after the returned string.
  Returns:
    A string occurring in fullstring exactly prefixed by prefix, and exactly
    terminated by suffix. For example, ("hello goodbye", "lo ", " bye") will
    return "good". If there is no such string, returns the empty string.
  """

  prefix_index = fullstring.find(prefix)
  if (prefix_index < 0):
    return ""
  result_start_index = prefix_index + len(prefix)
  suffix_index = fullstring.find(suffix, result_start_index)
  if (suffix_index < 0):
    return ""
  return fullstring[result_start_index:suffix_index]


def _search_sdk_output(output, sdkname):
  """Returns the sdk version given xcodebuild stdout and an sdkname."""
  return _search_string(output, "(%s" % sdkname, ")")


def _xcode_version_output(repository_ctx, name, version, aliases, developer_dir):
  """Returns a string containing an xcode_version build target."""
  build_contents = ""
  decorated_aliases = []
  error_msg = ""
  for alias in aliases:
    decorated_aliases.append("'%s'" % alias)
  xcodebuild_result = repository_ctx.execute(["xcrun", "xcodebuild", "-version", "-sdk"], 30,
                                             {"DEVELOPER_DIR": developer_dir})
  if (xcodebuild_result.return_code != 0):
    error_msg = (
        "Invoking xcodebuild failed, developer dir: {devdir} ," +
        "return code {code}, stderr: {err}, stdout: {out}").format(
            devdir=developer_dir,
            code=xcodebuild_result.return_code,
            err=xcodebuild_result.stderr,
            out=xcodebuild_result.stdout)
  ios_sdk_version = _search_sdk_output(xcodebuild_result.stdout, "iphoneos")
  tvos_sdk_version = _search_sdk_output(xcodebuild_result.stdout, "appletvos")
  macos_sdk_version = _search_sdk_output(xcodebuild_result.stdout, "macosx")
  watchos_sdk_version = _search_sdk_output(xcodebuild_result.stdout, "watchos")
  build_contents += "xcode_version(\n  name = '%s'," % name
  build_contents += "\n  version = '%s'," % version
  if aliases:
    build_contents += "\n  aliases = [%s]," % " ,".join(decorated_aliases)
  if ios_sdk_version:
    build_contents += "\n  default_ios_sdk_version = '%s'," % ios_sdk_version
  if tvos_sdk_version:
    build_contents += "\n  default_tvos_sdk_version = '%s'," % tvos_sdk_version
  if macos_sdk_version:
    build_contents += "\n  default_macos_sdk_version = '%s'," % macos_sdk_version
  if watchos_sdk_version:
    build_contents += "\n  default_watchos_sdk_version = '%s'," % watchos_sdk_version
  build_contents += "\n)\n"
  if error_msg:
    build_contents += "\n# Error: " + error_msg.replace("\n", " ") + "\n"
    print(error_msg)
  return build_contents


VERSION_CONFIG_STUB = "xcode_config(name = 'host_xcodes')"


def run_xcode_locator(repository_ctx, xcode_locator_src_label):
  """Generates xcode-locator from source and runs it.

  Builds xcode-locator in the current repository directory.
  Returns the standard output of running xcode-locator with -v, which will
  return information about locally installed Xcode toolchains and the versions
  they are associated with.

  This should only be invoked on a darwin OS, as xcode-locator cannot be built
  otherwise.

  Args:
    repository_ctx: The repository context.
    xcode_locator_src_label: The label of the source file for xcode-locator.
  Returns:
    A 2-tuple containing:
    output: A list representing installed xcode toolchain information. Each
        element of the list is a struct containing information for one installed
        toolchain. This is an empty list if there was an error building or
        running xcode-locator.
    err: An error string describing the error that occurred when attempting
        to build and run xcode-locator, or None if the run was successful.
  """
  xcodeloc_src_path = str(repository_ctx.path(xcode_locator_src_label))
  xcrun_result = repository_ctx.execute(["env", "-i", "xcrun", "clang", "-fobjc-arc", "-framework",
                                         "CoreServices", "-framework", "Foundation", "-o",
                                         "xcode-locator-bin", xcodeloc_src_path], 30)

  if (xcrun_result.return_code != 0):
    suggestion = ""
    if "Agreeing to the Xcode/iOS license" in xcrun_result.stderr:
      suggestion = ("(You may need to sign the xcode license." +
                    " Try running 'sudo xcodebuild -license')")
    error_msg = (
        "Generating xcode-locator-bin failed. {suggestion} " +
        "return code {code}, stderr: {err}, stdout: {out}").format(
            suggestion=suggestion,
            code=xcrun_result.return_code,
            err=xcrun_result.stderr,
            out=xcrun_result.stdout)
    return ([], error_msg.replace("\n", " "))

  xcode_locator_result = repository_ctx.execute(["./xcode-locator-bin", "-v"], 30)
  if (xcode_locator_result.return_code != 0):
    error_msg = (
        "Invoking xcode-locator failed, " +
        "return code {code}, stderr: {err}, stdout: {out}").format(
            code=xcode_locator_result.return_code,
            err=xcode_locator_result.stderr,
            out=xcode_locator_result.stdout)
    return ([], error_msg.replace("\n", " "))
  xcode_toolchains = []
  # xcode_dump is comprised of newlines with different installed xcode versions,
  # each line of the form <version>:<comma_separated_aliases>:<developer_dir>.
  xcode_dump = xcode_locator_result.stdout
  for xcodeversion in xcode_dump.split("\n"):
    if ":" in xcodeversion:
      infosplit = xcodeversion.split(":")
      toolchain = struct(
          version = infosplit[0],
          aliases = infosplit[1].split(","),
          developer_dir = infosplit[2]
      )
      xcode_toolchains.append(toolchain)
  return (xcode_toolchains, None)


def _darwin_build_file(repository_ctx):
  """Evaluates local system state to create xcode_config and xcode_version targets."""
  xcodebuild_result = repository_ctx.execute(["env", "-i", "xcrun", "xcodebuild", "-version"], 30)
  # "xcodebuild -version" failing may be indicative of no versions of xcode
  # installed, which is an acceptable machine configuration to have for using
  # bazel. Thus no print warning should be emitted here.
  if (xcodebuild_result.return_code != 0):
    error_msg = (
        "Running xcodebuild -version failed, " +
        "return code {code}, stderr: {err}, stdout: {out}").format(
            code=xcodebuild_result.return_code,
            err=xcodebuild_result.stderr,
            out=xcodebuild_result.stdout)
    return VERSION_CONFIG_STUB + "\n# Error: " + error_msg.replace("\n", " ") + "\n"

  (toolchains, xcodeloc_err) = run_xcode_locator(repository_ctx,
                                                 Label(repository_ctx.attr.xcode_locator))

  if xcodeloc_err:
    return VERSION_CONFIG_STUB + "\n# Error: " + xcodeloc_err + "\n"

  default_xcode_version = _search_string(xcodebuild_result.stdout, "Xcode ", "\n")
  default_xcode_target = ""
  target_names = []
  buildcontents = ""

  for toolchain in toolchains:
    version = toolchain.version
    aliases = toolchain.aliases
    developer_dir = toolchain.developer_dir
    target_name = "version%s" % version.replace(".", "_")
    buildcontents += _xcode_version_output(repository_ctx, target_name, version, aliases, developer_dir)
    target_names.append("':%s'" % target_name)
    if (version == default_xcode_version or default_xcode_version in aliases):
      default_xcode_target = target_name
  buildcontents += "xcode_config(name = 'host_xcodes',"
  if target_names:
    buildcontents += "\n  versions = [%s]," % ", ".join(target_names)
  if default_xcode_target:
    buildcontents += "\n  default = ':%s'," % default_xcode_target
  buildcontents += "\n)\n"
  return buildcontents


def _impl(repository_ctx):
  """Implementation for the local_config_xcode repository rule.

  Generates a BUILD file containing a root xcode_config target named 'host_xcodes',
  which points to an xcode_version target for each version of xcode installed on
  the local host machine. If no versions of xcode are present on the machine
  (for instance, if this is a non-darwin OS), creates a stub target.

  Args:
    repository_ctx: The repository context.
  """

  os_name = repository_ctx.os.name.lower()
  build_contents = "package(default_visibility = ['//visibility:public'])\n\n"
  if (os_name.startswith("mac os")):
    build_contents += _darwin_build_file(repository_ctx)
  else:
    build_contents += VERSION_CONFIG_STUB
  repository_ctx.file("BUILD", build_contents)

xcode_autoconf = repository_rule(
    implementation=_impl,
    local=True,
    attrs={
        "xcode_locator": attr.string(),
    }
)


def xcode_configure(xcode_locator_label):
  """Generates a repository containing host xcode version information."""
  xcode_autoconf(
      name="local_config_xcode",
      xcode_locator=xcode_locator_label
  )
