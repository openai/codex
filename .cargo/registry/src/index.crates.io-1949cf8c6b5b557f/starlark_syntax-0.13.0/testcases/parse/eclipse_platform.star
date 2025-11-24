# @generated
# Copyright 2017 The Bazel Authors. All rights reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
# http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# The Eclipse website provides SHA-512 but Bazel only support SHA256.
# Really Bazel should start supporting all "safe" checksum (and also
# drop support for SHA-1).
SHA256_SUM={
    # TODO(dmarting): we only support 4.5.2 right now because we need to
    # download all version of eclipse to provide those checksums...
    "4.5.2": {
        "macosx-cocoa-x86_64": "755f8a75075f6310a8d0453b5766a84aca2fcc687808341b7a657259230b490f",
        "linux-gtk-x86_64": "87f82b0c13c245ee20928557dbc4435657d1e029f72d9135683c8d585c69ba8d"
    }
}

def _get_file_url(version, platform, t):
  drop = "drops"
  if int(version.split(".", 1)[0]) >= 4:
    drop = "drops4"
  short_version = version.split("-", 1)[0]
  sha256 = ""
  if short_version in SHA256_SUM:
    if platform in SHA256_SUM[short_version]:
      sha256 = SHA256_SUM[short_version][platform]

  filename = "eclipse-SDK-%s-%s.%s" % (short_version, platform, t)
  file = "/eclipse/downloads/%s/R-%s/%s" % (
      drop,
      version,
      filename)
  # This is a mirror, original base url is http://www.eclipse.org/downloads/download.php?file=
  base_url = "https://storage.googleapis.com/bazel-mirror/download.eclipse.org"
  return (base_url + file, sha256)


def _eclipse_platform_impl(rctx):
  version = rctx.attr.version
  os_name = rctx.os.name.lower()
  if os_name.startswith("mac os"):
    platform = "macosx-cocoa-x86_64"
    t = "tar.gz"
  elif os_name.startswith("linux"):
    platform = "linux-gtk-x86_64"
    t = "tar.gz"
  else:
    fail("Cannot fetch Eclipse for platform %s" % rctx.os.name)
  url, sha256 = _get_file_url(version, platform, t)
  rctx.download_and_extract(url=url, type=t, sha256=sha256)
  rctx.file("BUILD.bazel", """
package(default_visibility = ["//visibility:public"])
filegroup(name = "platform", srcs = glob(["**"], exclude = ["BUILD.bazel", "BUILD"]))
filegroup(name = "launcher", srcs = glob(["**/plugins/org.eclipse.equinox.launcher_*.jar"]))
""")


eclipse_platform = repository_rule(
  implementation = _eclipse_platform_impl,
  attrs = {
    "version": attr.string(mandatory=True),
  }, local=False)
"""A repository for downloading the good version eclipse depending on the platform."""
