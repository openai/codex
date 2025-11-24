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

# Once nested repositories work, this file should cease to exist.

load("@io_bazel_rules_go//go/private:repository_tools.bzl", "go_repository_tools")
load("@io_bazel_rules_go//go/private:go_repository.bzl", "go_repository")
load('@io_bazel_rules_go//go/private:rules/stdlib.bzl', "go_stdlib")
load('@io_bazel_rules_go//go/toolchain:toolchains.bzl', "go_register_toolchains")
load("@io_bazel_rules_go//go/platform:list.bzl", "GOOS_GOARCH")

def go_rules_dependencies():
  """See /go/workspace.rst#go-rules-dependencies for full documentation."""

  # Needed for gazelle and wtool
  _maybe(native.http_archive,
      name = "com_github_bazelbuild_buildtools",
      # master, as of 2017-08-14
      url = "https://codeload.github.com/bazelbuild/buildtools/zip/799e530642bac55de7e76728fa0c3161484899f6",
      strip_prefix = "buildtools-799e530642bac55de7e76728fa0c3161484899f6",
      type = "zip",
  )

  # Needed for fetch repo
  _maybe(go_repository,
      name = "org_golang_x_tools",
      # release-branch.go1.9, as of 2017-08-25
      importpath = "golang.org/x/tools",
      urls = ["https://codeload.github.com/golang/tools/zip/5d2fd3ccab986d52112bf301d47a819783339d0e"],
      strip_prefix = "tools-5d2fd3ccab986d52112bf301d47a819783339d0e",
      type = "zip",
  )

  for goos, goarch in GOOS_GOARCH:
    _maybe(go_stdlib,
        name = "go_stdlib_{}_{}_cgo".format(goos, goarch),
        goos = goos,
        goarch = goarch,
        race = False,
        cgo = True,
    )
    _maybe(go_stdlib,
        name = "go_stdlib_{}_{}_pure".format(goos, goarch),
        goos = goos,
        goarch = goarch,
        race = False,
        cgo = False,
    )
    _maybe(go_stdlib,
        name = "go_stdlib_{}_{}_cgo_race".format(goos, goarch),
        goos = goos,
        goarch = goarch,
        race = True,
        cgo = True,
    )
    _maybe(go_stdlib,
        name = "go_stdlib_{}_{}_pure_race".format(goos, goarch),
        goos = goos,
        goarch = goarch,
        race = True,
        cgo = False,
    )

  _maybe(go_repository_tools,
      name = "io_bazel_rules_go_repository_tools",
  )

  # Proto dependencies
  _maybe(go_repository,
      name = "com_github_golang_protobuf",
      importpath = "github.com/golang/protobuf",
      commit = "1e59b77b52bf8e4b449a57e6f79f21226d571845",  # master, as of 2017-11-24
  )
  _maybe(native.http_archive,
      name = "com_google_protobuf",
      # v3.5.0, latest as of 2017-11-24
      url = "https://codeload.github.com/google/protobuf/zip/2761122b810fe8861004ae785cc3ab39f384d342",
      strip_prefix = "protobuf-2761122b810fe8861004ae785cc3ab39f384d342",
      type = "zip",
  )

  # Only used by deprecated go_proto_library implementation
  _maybe(native.http_archive,
      name = "com_github_google_protobuf",
      url = "https://github.com/google/protobuf/archive/v3.4.0.tar.gz",
      strip_prefix = "protobuf-3.4.0",
  )

  # GRPC dependencies
  _maybe(go_repository,
      name = "org_golang_x_net",
      commit = "a04bdaca5b32abe1c069418fb7088ae607de5bd0",  # master as of 2017-10-10
      importpath = "golang.org/x/net",
  )
  _maybe(go_repository,
      name = "org_golang_x_text",
      commit = "ab5ac5f9a8deb4855a60fab02bc61a4ec770bd49",  # v0.1.0, latest as of 2017-10-10
      importpath = "golang.org/x/text",
  )
  _maybe(go_repository,
      name = "org_golang_google_grpc",
      commit = "f92cdcd7dcdc69e81b2d7b338479a19a8723cfa3",  # v1.6.0, latest as of 2017-10-10
      importpath = "google.golang.org/grpc",
      build_file_proto_mode = "disable",  # use existing generated code
  )
  _maybe(go_repository,
      name = "org_golang_google_genproto",
      commit = "f676e0f3ac6395ff1a529ae59a6670878a8371a6",  # master on 2017-10-10
      importpath = "google.golang.org/genproto",
  )

  # Needed for examples
  _maybe(go_repository,
      name = "com_github_golang_glog",
      commit = "23def4e6c14b4da8ac2ed8007337bc5eb5007998",
      importpath = "github.com/golang/glog",
  )
  _maybe(go_repository,
      name = "com_github_jteeuwen_go_bindata",
      importpath = "github.com/jteeuwen/go-bindata",
      commit = "a0ff2567cfb70903282db057e799fd826784d41d",
  )


def _maybe(repo_rule, name, **kwargs):
  if name not in native.existing_rules():
    repo_rule(name=name, **kwargs)
