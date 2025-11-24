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

load("@io_bazel_rules_go//go/private:common.bzl", "env_execute")

def executable_extension(ctx):
  extension = ""
  if ctx.os.name.startswith('windows'):
    extension = ".exe"
  return extension

def _go_host_sdk_impl(ctx):
  path = _detect_host_sdk(ctx)
  _sdk_build_file(ctx)
  _local_sdk(ctx, path)
  _prepare(ctx)

go_host_sdk = repository_rule(_go_host_sdk_impl, environ = ["GOROOT"])

def _go_download_sdk_impl(ctx):
  if ctx.os.name == 'linux':
    res = ctx.execute(['uname', '-p'])
    if res.return_code == 0 and res.stdout == 's390x':
      host = "linux_s390x"
    else:
      # uname -p, -i, and -m can return wildly different values on different
      # distributions and versions. Always default to amd64.
      host = "linux_amd64"
  elif ctx.os.name == 'mac os x':
    host = "darwin_amd64"
  elif ctx.os.name.startswith('windows'):
    host = "windows_amd64"
  else:
    fail("Unsupported operating system: " + ctx.os.name)
  sdks = ctx.attr.sdks
  if host not in sdks: fail("Unsupported host {}".format(host))
  filename, sha256 = ctx.attr.sdks[host]
  _sdk_build_file(ctx)
  _remote_sdk(ctx, [url.format(filename) for url in ctx.attr.urls], ctx.attr.strip_prefix, sha256)
  _prepare(ctx)

go_download_sdk = repository_rule(_go_download_sdk_impl,
    attrs = {
        "sdks": attr.string_list_dict(),
        "urls": attr.string_list(default=["https://storage.googleapis.com/golang/{}"]),
        "strip_prefix": attr.string(default="go"),
    },
)

def _go_local_sdk_impl(ctx):
  _sdk_build_file(ctx)
  _local_sdk(ctx, ctx.attr.path)
  _prepare(ctx)

go_local_sdk = repository_rule(_go_local_sdk_impl,
    attrs = {
        "path": attr.string(),
    },
)

def _go_sdk_impl(ctx):
  urls = ctx.attr.urls
  if ctx.attr.url:
    print("DEPRECATED: use urls instead of url on go_sdk, {}".format(ctx.attr.url))
    urls = [ctx.attr.url] + urls
  if urls:
    if ctx.attr.path:
      fail("url and path cannot both be set on go_sdk, got {} and {}".format(urls, ctx.attr.path))
    _sdk_build_file(ctx)
    _remote_sdk(ctx, urls, ctx.attr.strip_prefix, ctx.attr.sha256)
  elif ctx.attr.path:
    print("DEPRECATED: go_sdk with a path, please use go_local_sdk")
    _sdk_build_file(ctx)
    _local_sdk(ctx, ctx.attr.path)
  else:
    print("DEPRECATED: go_sdk without path or urls, please use go_host_sdk")
    path = _detect_host_sdk(ctx)
    _sdk_build_file(ctx)
    _local_sdk(ctx, path)
  _prepare(ctx)


def _prepare(ctx):
  # Create a text file with a list of standard packages.
  # OPT: just list directories under src instead of running "go list". No
  # need to read all source files. We need a portable way to run code though.
  result = env_execute(ctx,
     arguments = ["bin/go", "list", "..."],
     environment = {"GOROOT": str(ctx.path("."))},
  )
  if result.return_code != 0:
    print(result.stderr)
    fail("failed to list standard packages")
  ctx.file("packages.txt", result.stdout)

go_sdk = repository_rule(
    implementation = _go_sdk_impl,
    attrs = {
        "path": attr.string(),
        "url": attr.string(),
        "urls": attr.string_list(),
        "strip_prefix": attr.string(default="go"),
        "sha256": attr.string(),
    },
)
"""See /go/toolchains.rst#go-sdk for full documentation."""

def _remote_sdk(ctx, urls, strip_prefix, sha256):
  ctx.download_and_extract(
      url = urls,
      stripPrefix = strip_prefix,
      sha256 = sha256,
  )

def _local_sdk(ctx, path):
  for entry in ["src", "pkg", "bin"]:
    ctx.symlink(path+"/"+entry, entry)

def _sdk_build_file(ctx):
  ctx.file("ROOT")
  ctx.template("BUILD.bazel",
      Label("@io_bazel_rules_go//go/private:BUILD.sdk.bazel"),
      executable = False,
  )

def _detect_host_sdk(ctx):
  root = "@invalid@"
  if "GOROOT" in ctx.os.environ:
    return ctx.os.environ["GOROOT"]
  res = ctx.execute(["go"+executable_extension(ctx), "env", "GOROOT"])
  if res.return_code:
    fail("Could not detect host go version")
  root = res.stdout.strip()
  if not root:
    fail("host go version failed to report it's GOROOT")
  return root
