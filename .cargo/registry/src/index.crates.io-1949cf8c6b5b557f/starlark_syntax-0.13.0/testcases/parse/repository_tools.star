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

load("@io_bazel_rules_go//go/private:go_repository.bzl", "go_repository", "env_execute")
load("@io_bazel_rules_go//go/private:toolchain.bzl", "executable_extension")

_GO_REPOSITORY_TOOLS_BUILD_FILE = """
package(default_visibility = ["//visibility:public"])

filegroup(
    name = "fetch_repo",
    srcs = ["bin/fetch_repo{extension}"],
)

filegroup(
    name = "gazelle",
    srcs = ["bin/gazelle{extension}"],
)
"""

def _go_repository_tools_impl(ctx):
  # We work this out here because you can't use a toolchain from a repository rule
  # TODO: This is an ugly non sustainable hack, we need to kill repository tools.

  extension = executable_extension(ctx)
  go_tool = ctx.path(Label("@go_sdk//:bin/go{}".format(extension)))

  x_tools_commit = "3d92dd60033c312e3ae7cac319c792271cf67e37"
  x_tools_path = ctx.path('tools-' + x_tools_commit)
  buildtools_path = ctx.path(ctx.attr._buildtools).dirname
  go_tools_path = ctx.path(ctx.attr._tools).dirname

  # We have to download this directly because the normal version is based on go_repository
  # and thus requires the gazelle we build in here to generate it's BUILD files
  # The commit used here should match the one in repositories.bzl
  ctx.download_and_extract(
      url = "https://codeload.github.com/golang/tools/zip/" + x_tools_commit,
      type = "zip",
  )

  # Build something that looks like a normal GOPATH so go install will work
  ctx.symlink(x_tools_path, "src/golang.org/x/tools")
  ctx.symlink(buildtools_path, "src/github.com/bazelbuild/buildtools")
  ctx.symlink(go_tools_path, "src/github.com/bazelbuild/rules_go/go/tools")
  env = {
    'GOROOT': str(go_tool.dirname.dirname),
    'GOPATH': str(ctx.path('')),
  }

  # build all the repository tools
  for tool, importpath in (
      ("gazelle", 'github.com/bazelbuild/rules_go/go/tools/gazelle/gazelle'),
      ("fetch_repo", 'github.com/bazelbuild/rules_go/go/tools/fetch_repo'),
  ):
    result = env_execute(ctx, [go_tool, "install", importpath], environment = env)
    if result.return_code:
        fail("failed to build {}: {}".format(tool, result.stderr))

  # add a build file to export the tools
  ctx.file('BUILD.bazel', _GO_REPOSITORY_TOOLS_BUILD_FILE.format(extension=executable_extension(ctx)), False)

go_repository_tools = repository_rule(
    _go_repository_tools_impl,
    attrs = {
        "linux_sdk": attr.string(),
        "darwin_sdk": attr.string(),
        "_tools": attr.label(
            default = Label("//go/tools:BUILD.bazel"),
            allow_files = True,
            single_file = True,
        ),
        "_buildtools": attr.label(
            default = Label("@com_github_bazelbuild_buildtools//:WORKSPACE"),
            allow_files = True,
            single_file = True,
        ),
    },
    environ = ["TMP"],
)
