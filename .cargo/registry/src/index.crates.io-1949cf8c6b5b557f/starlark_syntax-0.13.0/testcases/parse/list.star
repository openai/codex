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

GOOS = {
    "android": None,
    "darwin": "@bazel_tools//platforms:osx",
    "dragonfly": None,
    "freebsd": "@bazel_tools//platforms:freebsd",
    "linux": "@bazel_tools//platforms:linux",
    "nacl": None,
    "netbsd": None,
    "openbsd": None,
    "plan9": None,
    "solaris": None,
    "windows": "@bazel_tools//platforms:windows",
}

GOARCH = {
    "386": "@bazel_tools//platforms:x86_32",
    "amd64": "@bazel_tools//platforms:x86_64",
    "amd64p32": None,
    "arm": "@bazel_tools//platforms:arm",
    "arm64": None,
    "mips": None,
    "mips64": None,
    "mips64le": None,
    "mipsle": None,
    "ppc64": "@bazel_tools//platforms:ppc",
    "ppc64le": None,
    "s390x": "@bazel_tools//platforms:s390x",
}

GOOS_GOARCH = (
    ("android", "386"),
    ("android", "amd64"),
    ("android", "arm"),
    ("android", "arm64"),
    ("darwin", "386"),
    ("darwin", "amd64"),
    ("darwin", "arm"),
    ("darwin", "arm64"),
    ("dragonfly", "amd64"),
    ("freebsd", "386"),
    ("freebsd", "amd64"),
    ("freebsd", "arm"),
    ("linux", "386"),
    ("linux", "amd64"),
    ("linux", "arm"),
    ("linux", "arm64"),
    ("linux", "mips"),
    ("linux", "mips64"),
    ("linux", "mips64le"),
    ("linux", "mipsle"),
    ("linux", "ppc64"),
    ("linux", "ppc64le"),
    ("linux", "s390x"),
    ("nacl", "386"),
    ("nacl", "amd64p32"),
    ("nacl", "arm"),
    ("netbsd", "386"),
    ("netbsd", "amd64"),
    ("netbsd", "arm"),
    ("openbsd", "386"),
    ("openbsd", "amd64"),
    ("openbsd", "arm"),
    ("plan9", "386"),
    ("plan9", "amd64"),
    ("plan9", "arm"),
    ("solaris", "amd64"),
    ("windows", "386"),
    ("windows", "amd64"),
)

def declare_config_settings():
  for goos in GOOS:
    native.config_setting(
        name = goos,
        constraint_values = ["//go/toolchain:" + goos],
    )
  for goarch in GOARCH:
    native.config_setting(
        name = goarch,
        constraint_values = ["//go/toolchain:" + goarch],
    )
  for goos, goarch in GOOS_GOARCH:
    native.config_setting(
        name = goos + "_" + goarch,
        constraint_values = [
            "//go/toolchain:" + goos,
            "//go/toolchain:" + goarch,
        ],
    )
