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

##############################
# Generated file, do not edit!
##############################

load("@io_bazel_rules_go//go/private:go_repository.bzl", "go_repository")

def _maybe(repo_rule, name, **kwargs):
  if name not in native.existing_rules():
    repo_rule(name=name, **kwargs)

def popular_repos():
  _maybe(
    go_repository,
    name="org_golang_x_crypto",
    importpath="golang.org/x/crypto",
    strip_prefix="crypto-81e90905daefcd6fd217b62423c0908922eadb30",
    type="zip",
    urls=['https://codeload.github.com/golang/crypto/zip/81e90905daefcd6fd217b62423c0908922eadb30'],
  )
  _maybe(
    go_repository,
    name="org_golang_x_net",
    importpath="golang.org/x/net",
    commit="57efc9c3d9f91fb3277f8da1cff370539c4d3dc5",
  )
  _maybe(
    go_repository,
    name="org_golang_x_sys",
    importpath="golang.org/x/sys",
    commit="0b25a408a50076fbbcae6b7ac0ea5fbb0b085e79",
  )
  _maybe(
    go_repository,
    name="org_golang_x_text",
    importpath="golang.org/x/text",
    commit="a9a820217f98f7c8a207ec1e45a874e1fe12c478",
  )
  _maybe(
    go_repository,
    name="org_golang_x_tools",
    importpath="golang.org/x/tools",
    commit="663269851cdddc898f963782f74ea574bcd5c814",
  )
  _maybe(
    go_repository,
    name="org_golang_google_grpc",
    importpath="google.golang.org/grpc",
    commit="3f10311ccf076b6b7cba28273df3290d42e60982",
    build_file_proto_mode="disable",
  )
  _maybe(
    go_repository,
    name="com_github_mattn_go_sqlite3",
    importpath="github.com/mattn/go-sqlite3",
    commit="83772a7051f5e30d8e59746a9e43dfa706b72f3b",
  )
