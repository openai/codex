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
"""Rules for downloading files and archives over HTTP.

### Setup

To use these rules, load them in your `WORKSPACE` file as follows:

```python
load(
    "@bazel_tools//tools/build_defs/repo:http.bzl",
    "http_archive",
    "http_file",
)
```

These rules are improved versions of the native http rules and will eventually
replace the native rules.
"""


def _http_archive_impl(ctx):
  """Implementation of the http_archive rule."""
  if ctx.attr.build_file and ctx.attr.build_file_content:
    ctx.fail("Only one of build_file and build_file_content can be provided.")

  ctx.download_and_extract(ctx.attr.urls, "", ctx.attr.sha256, ctx.attr.type,
                           ctx.attr.strip_prefix)
  ctx.file("WORKSPACE", "workspace(name = \"{name}\")\n".format(name=ctx.name))
  if ctx.attr.build_file:
    print("ctx.attr.build_file %s" % str(ctx.attr.build_file))
    ctx.symlink(ctx.attr.build_file, "BUILD")
  else:
    ctx.file("BUILD", ctx.attr.build_file_content)


_HTTP_FILE_BUILD = """
package(default_visibility = ["//visibility:public"])

filegroup(
    name = "file",
    srcs = ["downloaded"],
)
"""

def _http_file_impl(ctx):
  """Implementation of the http_file rule."""
  ctx.download(ctx.attr.urls, "file/downloaded", ctx.attr.sha256,
               ctx.attr.executable)
  ctx.file("WORKSPACE", "workspace(name = \"{name}\")".format(name=ctx.name))
  ctx.file("file/BUILD", _HTTP_FILE_BUILD)


_http_archive_attrs = {
    "urls": attr.string_list(mandatory=True),
    "sha256": attr.string(),
    "strip_prefix": attr.string(),
    "type": attr.string(),
    "build_file": attr.label(),
    "build_file_content": attr.string(),
}


http_archive = repository_rule(
    implementation = _http_archive_impl,
    attrs = _http_archive_attrs,
)
"""Downloads a Bazel repository as a compressed archive file, decompresses it,
and makes its targets available for binding.

The repository should already contain a BUILD file. If it does not, use
`new_http_archive` instead.

It supports the following file extensions: `"zip"`, `"jar"`, `"war"`,
`"tar.gz"`, `"tgz"`, `"tar.xz"`, and `tar.bz2`.

Examples:
  Suppose the current repository contains the source code for a chat program,
  rooted at the directory `~/chat-app`. It needs to depend on an SSL library
  which is available from http://example.com/openssl.zip. This `.zip` file
  contains the following directory structure:

  ```
  WORKSPACE
  src/
    openssl.cc
    openssl.h
  ```

  In the local repository, the user creates a `openssl.BUILD` file which
  contains the following target definition:

  ```python
  cc_library(
      name = "openssl-lib",
      srcs = ["src/openssl.cc"],
      hdrs = ["src/openssl.h"],
  )
  ```

  Targets in the `~/chat-app` repository can depend on this target if the
  following lines are added to `~/chat-app/WORKSPACE`:

  ```python
  http_archive(
      name = "my_ssl",
      urls = ["http://example.com/openssl.zip"],
      sha256 = "03a58ac630e59778f328af4bcc4acb4f80208ed4",
      build_file = "//:openssl.BUILD",
  )
  ```

  Then targets would specify `@my_ssl//:openssl-lib` as a dependency.

Args:
  name: A unique name for this rule.
  build_file: The file to use as the `BUILD` file for this repository.

    Either `build_file` or `build_file_content` can be specified.

    This attribute is a label relative to the main workspace. The file does not
    need to be named `BUILD`, but can be (something like `BUILD.new-repo-name`
    may work well for distinguishing it from the repository's actual `BUILD`
    files.

  build_file_content: The content for the BUILD file for this repository.

    Either `build_file` or `build_file_content` can be specified.
  sha256: The expected SHA-256 of the file downloaded.

    This must match the SHA-256 of the file downloaded. _It is a security risk
    to omit the SHA-256 as remote files can change._ At best omitting this
    field will make your build non-hermetic. It is optional to make development
    easier but should be set before shipping.
  strip_prefix: A directory prefix to strip from the extracted files.

    Many archives contain a top-level directory that contains all of the useful
    files in archive. Instead of needing to specify this prefix over and over
    in the `build_file`, this field can be used to strip it from all of the
    extracted files.

    For example, suppose you are using `foo-lib-latest.zip`, which contains the
    directory `foo-lib-1.2.3/` under which there is a `WORKSPACE` file and are
    `src/`, `lib/`, and `test/` directories that contain the actual code you
    wish to build. Specify `strip_prefix = "foo-lib-1.2.3"` to use the
    `foo-lib-1.2.3` directory as your top-level directory.

    Note that if there are files outside of this directory, they will be
    discarded and inaccessible (e.g., a top-level license file). This includes
    files/directories that start with the prefix but are not in the directory
    (e.g., `foo-lib-1.2.3.release-notes`). If the specified prefix does not
    match a directory in the archive, Bazel will return an error.
  type: The archive type of the downloaded file.

    By default, the archive type is determined from the file extension of the
    URL. If the file has no extension, you can explicitly specify one of the
    following: `"zip"`, `"jar"`, `"war"`, `"tar.gz"`, `"tgz"`, `"tar.xz"`,
    or `tar.bz2`.
  urls: A URL to a file that will be made available to Bazel.

    This must be an file, http or https URL. Redirections are followed.
    Authentication is not supported.
"""


http_file = repository_rule(
    implementation = _http_file_impl,
    attrs = {
        "executable": attr.bool(),
        "sha256": attr.string(),
        "urls": attr.string_list(mandatory=True),
    },
)
"""Downloads a file from a URL and makes it available to be used as a file
group.

Examples:
  Suppose you need to have a debian package for your custom rules. This package
  is available from http://example.com/package.deb. Then you can add to your
  WORKSPACE file:

  ```python
  http_file(
      name = "my_deb",
      urls = ["http://example.com/package.deb"],
      sha256 = "03a58ac630e59778f328af4bcc4acb4f80208ed4",
  )
  ```

  Targets would specify `@my_deb//file` as a dependency to depend on this file.

Args:
  name: A unique name for this rule.
  executable: If the downloaded file should be made executable. Defaults to
    False.
  sha256: The expected SHA-256 of the file downloaded.

    This must match the SHA-256 of the file downloaded. _It is a security risk
    to omit the SHA-256 as remote files can change._ At best omitting this
    field will make your build non-hermetic. It is optional to make development
    easier but should be set before shipping.
  urls: A URL to a file that will be made available to Bazel.

    This must be an file, http, or https URL. Redirections are followed.
    Authentication is not supported.
"""
