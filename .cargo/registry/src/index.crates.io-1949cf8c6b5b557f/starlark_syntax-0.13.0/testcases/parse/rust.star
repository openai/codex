# @generated
# Copyright 2015 The Bazel Authors. All rights reserved.
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

"""Rust Rules

These build rules are used for building [Rust][rust] projects with Bazel.

[rust]: http://www.rust-lang.org/

### Setup

To use the Rust rules, add the following to your `WORKSPACE` file to add the
external repositories for the Rust toolchain:

```python
http_archive(
    name = "io_bazel_rules_rust",
    sha256 = "aa7ad550e2960143835c6a7d3bbc29e313aedf89ea879e5465e97f5d6a19e7f5",
    strip_prefix = "rules_rust-0.0.5",
    urls = [
        "http://bazel-mirror.storage.googleapis.com/github.com/bazelbuild/rules_rust/archive/0.0.5.tar.gz",
        "https://github.com/bazelbuild/rules_rust/archive/0.0.5.tar.gz",
    ],
)
load("@io_bazel_rules_rust//rust:rust.bzl", "rust_repositories")

rust_repositories()
```

### Roadmap

* Add `rust_toolchain` rule to make it easy to use a custom Rust toolchain.
* Add tool for taking `Cargo.toml` and generating a `WORKSPACE` file with
  workspace rules for pulling external dependencies.
* Improve expressiveness of features and support for [Cargo's feature
  groups](http://doc.crates.io/manifest.html#the-[features]-section).
* Add `cargo_crate` workspace rule for pulling crates from
  [Cargo](https://crates.io/).
"""

RUST_FILETYPE = FileType([".rs"])

A_FILETYPE = FileType([".a"])

LIBRARY_CRATE_TYPES = [
    "lib",
    "rlib",
    "dylib",
    "staticlib",
]

# Used by rust_doc
HTML_MD_FILETYPE = FileType([
    ".html",
    ".md",
])

CSS_FILETYPE = FileType([".css"])

ZIP_PATH = "/usr/bin/zip"

def _path_parts(path):
  """Takes a path and returns a list of its parts with all "." elements removed.

  The main use case of this function is if one of the inputs to _relative()
  is a relative path, such as "./foo".

  Args:
    path_parts: A list containing parts of a path.

  Returns:
    Returns a list containing the path parts with all "." elements removed.
  """
  path_parts = path.split("/")
  return [part for part in path_parts if part != "."]

def _relative(src_path, dest_path):
  """Returns the relative path from src_path to dest_path."""
  src_parts = _path_parts(src_path)
  dest_parts = _path_parts(dest_path)
  n = 0
  done = False
  for src_part, dest_part in zip(src_parts, dest_parts):
    if src_part != dest_part:
      break
    n += 1

  relative_path = ""
  for i in range(n, len(src_parts)):
    relative_path += "../"
  relative_path += "/".join(dest_parts[n:])

  return relative_path

def _create_setup_cmd(lib, deps_dir, in_runfiles):
  """
  Helper function to construct a command for symlinking a library into the
  deps directory.
  """
  lib_path = lib.short_path if in_runfiles else lib.path
  return (
      "ln -sf " + _relative(deps_dir, lib_path) + " " +
      deps_dir + "/" + lib.basename + "\n"
  )

def _setup_deps(deps, name, working_dir, allow_cc_deps=False,
                in_runfiles=False):
  """
  Walks through dependencies and constructs the necessary commands for linking
  to all the necessary dependencies.

  Args:
    deps: List of Labels containing deps from ctx.attr.deps.
    name: Name of the current target.
    working_dir: The output directory for the current target's outputs.
    allow_cc_deps: True if the current target is allowed to depend on cc_library
        targets, false otherwise.
    in_runfiles: True if the setup commands will be run in a .runfiles
        directory. In this case, the working dir should be '.', and the deps
        will be symlinked into the .deps dir from the runfiles tree.

  Returns:
    Returns a struct containing the following fields:
      libs:
      transitive_libs:
      setup_cmd:
      search_flags:
      link_flags:
  """
  deps_dir = working_dir + "/" + name + ".deps"
  setup_cmd = ["rm -rf " + deps_dir + "; mkdir " + deps_dir + "\n"]

  has_rlib = False
  has_native = False

  libs = set_which_is_banned()
  transitive_libs = set_which_is_banned()
  symlinked_libs = set_which_is_banned()
  link_flags = []
  for dep in deps:
    if hasattr(dep, "rust_lib"):
      # This dependency is a rust_library
      libs += [dep.rust_lib]
      transitive_libs += [dep.rust_lib] + dep.transitive_libs
      symlinked_libs += [dep.rust_lib] + dep.transitive_libs
      link_flags += [(
          "--extern " + dep.label.name + "=" +
          deps_dir + "/" + dep.rust_lib.basename
      )]
      has_rlib = True

    elif hasattr(dep, "cc"):
      if not allow_cc_deps:
        fail("Only rust_library, rust_binary, and rust_test targets can " +
             "depend on cc_library")

      # This dependency is a cc_library
      native_libs = A_FILETYPE.filter(dep.cc.libs)
      libs += native_libs
      transitive_libs += native_libs
      symlinked_libs += native_libs
      link_flags += ["-l static=" + dep.label.name]
      has_native = True

    else:
      fail("rust_library, rust_binary and rust_test targets can only depend " +
           "on rust_library or cc_library targets.")

  for symlinked_lib in symlinked_libs:
    setup_cmd += [_create_setup_cmd(symlinked_lib, deps_dir, in_runfiles)]

  search_flags = []
  if has_rlib:
    search_flags += ["-L dependency=%s" % deps_dir]
  if has_native:
    search_flags += ["-L native=%s" % deps_dir]

  return struct(
      libs = list(libs),
      transitive_libs = list(transitive_libs),
      setup_cmd = setup_cmd,
      search_flags = search_flags,
      link_flags = link_flags)

def _get_features_flags(features):
  """
  Constructs a string containing the feature flags from the features specified
  in the features attribute.
  """
  features_flags = []
  for feature in features:
    features_flags += ["--cfg feature=\\\"%s\\\"" % feature]
  return features_flags

def _get_dirname(short_path):
  return short_path[0:short_path.rfind('/')]

def _rust_toolchain(ctx):
  return struct(
      rustc_path = ctx.file._rustc.path,
      rustc_lib_path = ctx.files._rustc_lib[0].dirname,
      rustc_lib_short_path = _get_dirname(ctx.files._rustc_lib[0].short_path),
      rust_lib_path = ctx.files._rust_lib[0].dirname,
      rust_lib_short_path = _get_dirname(ctx.files._rust_lib[0].short_path),
      rustdoc_path = ctx.file._rustdoc.path,
      rustdoc_short_path = ctx.file._rustdoc.short_path)

def _build_rustc_command(ctx, crate_name, crate_type, src, output_dir,
                         depinfo, rust_flags=[]):
  """Builds the rustc command.

  Constructs the rustc command used to build the current target.

  Args:
    ctx: The ctx object for the current target.
    crate_type: The type of crate to build ("lib" or "bin")
    src: The File object for crate root source file ("lib.rs" or "main.rs")
    output_dir: The output directory for the target.
    depinfo: Struct containing information about dependencies as returned by
        _setup_deps

  Return:
    String containing the rustc command.
  """

  # Paths to the Rust compiler and standard libraries.
  toolchain = _rust_toolchain(ctx)

  # Paths to cc (for linker) and ar
  cpp_fragment = ctx.fragments.cpp
  cc = cpp_fragment.compiler_executable
  ar = cpp_fragment.ar_executable
  # Currently, the CROSSTOOL config for darwin sets ar to "libtool". Because
  # rust uses ar-specific flags, use /usr/bin/ar in this case.
  # TODO(dzc): This is not ideal. Remove this workaround once ar_executable
  # always points to an ar binary.
  ar_str = "%s" % ar
  if ar_str.find("libtool", 0) != -1:
    ar = "/usr/bin/ar"

  # Construct features flags
  features_flags = _get_features_flags(ctx.attr.crate_features)

  return " ".join(
      ["set -e;"] +
      depinfo.setup_cmd +
      [
          "LD_LIBRARY_PATH=%s" % toolchain.rustc_lib_path,
          "DYLD_LIBRARY_PATH=%s" % toolchain.rustc_lib_path,
          toolchain.rustc_path,
          src.path,
          "--crate-name %s" % crate_name,
          "--crate-type %s" % crate_type,
          "-C opt-level=3",
          "--codegen ar=%s" % ar,
          "--codegen linker=%s" % cc,
          "--codegen link-args='%s'" % ' '.join(cpp_fragment.link_options),
          "-L all=%s" % toolchain.rust_lib_path,
          "--out-dir %s" % output_dir,
          "--emit=dep-info,link",
      ] +
      features_flags +
      rust_flags +
      depinfo.search_flags +
      depinfo.link_flags +
      ctx.attr.rustc_flags)

def _find_crate_root_src(srcs, file_names=["lib.rs"]):
  """Finds the source file for the crate root."""
  if len(srcs) == 1:
    return srcs[0]
  for src in srcs:
    if src.basename in file_names:
      return src
  fail("No %s source file found." % " or ".join(file_names), "srcs")

def _crate_root_src(ctx, file_names=["lib.rs"]):
  if ctx.file.crate_root == None:
    return _find_crate_root_src(ctx.files.srcs, file_names)
  else:
    return ctx.file.crate_root

def _rust_library_impl(ctx):
  """
  Implementation for rust_library Skylark rule.
  """

  # Find lib.rs
  lib_rs = _crate_root_src(ctx)

  # Validate crate_type
  crate_type = ""
  if ctx.attr.crate_type != "":
    if ctx.attr.crate_type not in LIBRARY_CRATE_TYPES:
      fail("Invalid crate_type for rust_library. Allowed crate types are: %s"
           % " ".join(LIBRARY_CRATE_TYPES), "crate_type")
    crate_type += ctx.attr.crate_type
  else:
    crate_type += "lib"

  # Output library
  rust_lib = ctx.outputs.rust_lib
  output_dir = rust_lib.dirname

  # Dependencies
  depinfo = _setup_deps(ctx.attr.deps,
                        ctx.label.name,
                        output_dir,
                        allow_cc_deps=True)

  # Build rustc command
  cmd = _build_rustc_command(
      ctx = ctx,
      crate_name = ctx.label.name,
      crate_type = crate_type,
      src = lib_rs,
      output_dir = output_dir,
      depinfo = depinfo)

  # Compile action.
  compile_inputs = (
      ctx.files.srcs +
      ctx.files.data +
      depinfo.libs +
      depinfo.transitive_libs +
      [ctx.file._rustc] +
      ctx.files._rustc_lib +
      ctx.files._rust_lib +
      ctx.files._crosstool)

  ctx.action(
      inputs = compile_inputs,
      outputs = [rust_lib],
      mnemonic = 'Rustc',
      command = cmd,
      use_default_shell_env = True,
      progress_message = ("Compiling Rust library %s (%d files)"
                          % (ctx.label.name, len(ctx.files.srcs))))

  return struct(
      files = set_which_is_banned([rust_lib]),
      crate_type = crate_type,
      crate_root = lib_rs,
      rust_srcs = ctx.files.srcs,
      rust_deps = ctx.attr.deps,
      transitive_libs = depinfo.transitive_libs,
      rust_lib = rust_lib)

def _rust_binary_impl(ctx):
  """Implementation for rust_binary Skylark rule."""

  # Find main.rs.
  main_rs = _crate_root_src(ctx, ["main.rs"])

  # Output binary
  rust_binary = ctx.outputs.executable
  output_dir = rust_binary.dirname

  # Dependencies
  depinfo = _setup_deps(ctx.attr.deps,
                        ctx.label.name,
                        output_dir,
                        allow_cc_deps=False)

  # Build rustc command.
  cmd = _build_rustc_command(ctx = ctx,
                             crate_name = ctx.label.name,
                             crate_type = "bin",
                             src = main_rs,
                             output_dir = output_dir,
                             depinfo = depinfo)

  # Compile action.
  compile_inputs = (
      ctx.files.srcs +
      ctx.files.data +
      depinfo.libs +
      depinfo.transitive_libs +
      [ctx.file._rustc] +
      ctx.files._rustc_lib +
      ctx.files._rust_lib +
      ctx.files._crosstool)

  ctx.action(
      inputs = compile_inputs,
      outputs = [rust_binary],
      mnemonic = "Rustc",
      command = cmd,
      use_default_shell_env = True,
      progress_message = ("Compiling Rust binary %s (%d files)"
                          % (ctx.label.name, len(ctx.files.srcs))))

  return struct(rust_srcs = ctx.files.srcs,
                crate_root = main_rs,
                rust_deps = ctx.attr.deps)

def _rust_test_common(ctx, test_binary):
  """Builds a Rust test binary.

  Args:
      ctx: The ctx object for the current target.
      test_binary: The File object for the test binary.
  """
  output_dir = test_binary.dirname

  if len(ctx.attr.deps) == 1 and len(ctx.files.srcs) == 0:
    # Target has a single dependency but no srcs. Build the test binary using
    # the dependency's srcs.
    dep = ctx.attr.deps[0]
    crate_type = getattr(dep, "crate_type", "bin")
    target = struct(name = ctx.label.name,
                    srcs = dep.rust_srcs,
                    deps = dep.rust_deps,
                    crate_root = dep.crate_root,
                    crate_type = crate_type)
  else:
    # Target is a standalone crate. Build the test binary as its own crate.
    target = struct(name = ctx.label.name,
                    srcs = ctx.files.srcs,
                    deps = ctx.attr.deps,
                    crate_root = _crate_root_src(ctx),
                    crate_type = "lib")

  # Get information about dependencies
  depinfo = _setup_deps(target.deps,
                        target.name,
                        output_dir,
                        allow_cc_deps=True)

  cmd = _build_rustc_command(ctx = ctx,
                             crate_name = test_binary.basename,
                             crate_type = target.crate_type,
                             src = target.crate_root,
                             output_dir = output_dir,
                             depinfo = depinfo,
                             rust_flags = ["--test"])

  compile_inputs = (target.srcs +
                    depinfo.libs +
                    depinfo.transitive_libs +
                    [ctx.file._rustc] +
                    ctx.files._rustc_lib +
                    ctx.files._rust_lib +
                    ctx.files._crosstool)

  ctx.action(
      inputs = compile_inputs,
      outputs = [test_binary],
      mnemonic = "RustcTest",
      command = cmd,
      use_default_shell_env = True,
      progress_message = ("Compiling Rust test %s (%d files)"
                          % (ctx.label.name, len(target.srcs))))

def _rust_test_impl(ctx):
  """
  Implementation for rust_test Skylark rule.
  """
  _rust_test_common(ctx, ctx.outputs.executable)

def _rust_bench_test_impl(ctx):
  """Implementation for the rust_bench_test Skylark rule."""
  rust_bench_test = ctx.outputs.executable
  test_binary = ctx.new_file(ctx.configuration.bin_dir,
                             "%s_bin" % rust_bench_test.basename)
  _rust_test_common(ctx, test_binary)

  ctx.file_action(
      output = rust_bench_test,
      content = " ".join([
          "#!/bin/bash\n",
          "set -e\n",
          "%s --bench\n" % test_binary.short_path]),
      executable = True)

  runfiles = ctx.runfiles(files = [test_binary], collect_data = True)
  return struct(runfiles = runfiles)

def _build_rustdoc_flags(ctx):
  """Collects the rustdoc flags."""
  doc_flags = []
  doc_flags += [
      "--markdown-css %s" % css.path for css in ctx.files.markdown_css]
  if hasattr(ctx.file, "html_in_header"):
    doc_flags += ["--html-in-header %s" % ctx.file.html_in_header.path]
  if hasattr(ctx.file, "html_before_content"):
    doc_flags += ["--html-before-content %s" %
                  ctx.file.html_before_content.path]
  if hasattr(ctx.file, "html_after_content"):
    doc_flags += ["--html-after-content %s"]
  return doc_flags

def _rust_doc_impl(ctx):
  """Implementation of the rust_doc rule."""
  rust_doc_zip = ctx.outputs.rust_doc_zip

  # Gather attributes about the rust_library target to generated rustdocs for.
  target = struct(name = ctx.label.name,
                  srcs = ctx.attr.dep.rust_srcs,
                  deps = ctx.attr.dep.rust_deps,
                  crate_root = ctx.attr.dep.crate_root)

  # Find lib.rs
  lib_rs = (_find_crate_root_src(target.srcs, ["lib.rs", "main.rs"])
            if target.crate_root == None else target.crate_root)

  # Get information about dependencies
  output_dir = rust_doc_zip.dirname
  depinfo = _setup_deps(target.deps,
                        target.name,
                        output_dir,
                        allow_cc_deps=False)

  # Rustdoc flags.
  doc_flags = _build_rustdoc_flags(ctx)

  # Build rustdoc command.
  toolchain = _rust_toolchain(ctx)
  docs_dir = rust_doc_zip.dirname + "/_rust_docs"
  doc_cmd = " ".join(
      ["set -e;"] +
      depinfo.setup_cmd + [
          "rm -rf %s;" % docs_dir,
          "mkdir %s;" % docs_dir,
          "LD_LIBRARY_PATH=%s" % toolchain.rustc_lib_path,
          "DYLD_LIBRARY_PATH=%s" % toolchain.rustc_lib_path,
          toolchain.rustdoc_path,
          lib_rs.path,
          "--crate-name %s" % target.name,
          "-L all=%s" % toolchain.rust_lib_path,
          "-o %s" % docs_dir,
      ] +
      doc_flags +
      depinfo.search_flags +
      depinfo.link_flags + [
          "&&",
          "(cd %s" % docs_dir,
          "&&",
          ZIP_PATH,
          "-qR",
          rust_doc_zip.basename,
          "$(find . -type f) )",
          "&&",
          "mv %s/%s %s" % (docs_dir, rust_doc_zip.basename, rust_doc_zip.path),
      ])

  # Rustdoc action
  rustdoc_inputs = (target.srcs +
                    depinfo.libs +
                    [ctx.file._rustdoc] +
                    ctx.files._rustc_lib +
                    ctx.files._rust_lib)

  ctx.action(
      inputs = rustdoc_inputs,
      outputs = [rust_doc_zip],
      mnemonic = "Rustdoc",
      command = doc_cmd,
      use_default_shell_env = True,
      progress_message = ("Generating rustdoc for %s (%d files)"
                          % (target.name, len(target.srcs))))

def _rust_doc_test_impl(ctx):
  """Implementation for the rust_doc_test rule."""
  rust_doc_test = ctx.outputs.executable

  # Gather attributes about the rust_library target to generated rustdocs for.
  target = struct(name = ctx.label.name,
                  srcs = ctx.attr.dep.rust_srcs,
                  deps = ctx.attr.dep.rust_deps,
                  crate_root = ctx.attr.dep.crate_root)

  # Find lib.rs
  lib_rs = (_find_crate_root_src(target.srcs, ["lib.rs", "main.rs"])
            if target.crate_root == None else target.crate_root)

  # Get information about dependencies
  output_dir = rust_doc_test.dirname
  depinfo = _setup_deps(target.deps,
                        target.name,
                        working_dir=".",
                        allow_cc_deps=False,
                        in_runfiles=True)

  # Construct rustdoc test command, which will be written to a shell script
  # to be executed to run the test.
  toolchain = _rust_toolchain(ctx)
  doc_test_cmd = " ".join(
      ["#!/bin/bash\n"] +
      ["set -e\n"] +
      depinfo.setup_cmd +
      [
          "LD_LIBRARY_PATH=%s" % toolchain.rustc_lib_short_path,
          "DYLD_LIBRARY_PATH=%s" % toolchain.rustc_lib_short_path,
          toolchain.rustdoc_short_path,
          "-L all=%s" % toolchain.rust_lib_short_path,
          lib_rs.path,
      ] +
      depinfo.search_flags +
      depinfo.link_flags)

  ctx.file_action(output = rust_doc_test,
                  content = doc_test_cmd,
                  executable = True)

  doc_test_inputs = (target.srcs +
                     depinfo.libs +
                     depinfo.transitive_libs +
                     [ctx.file._rustdoc] +
                     ctx.files._rustc_lib +
                     ctx.files._rust_lib)

  runfiles = ctx.runfiles(files = doc_test_inputs, collect_data = True)
  return struct(runfiles = runfiles)

_rust_common_attrs = {
    "srcs": attr.label_list(allow_files = RUST_FILETYPE),
    "crate_root": attr.label(
        allow_files = RUST_FILETYPE,
        single_file = True,
    ),
    "data": attr.label_list(
        allow_files = True,
        cfg = "data",
    ),
    "deps": attr.label_list(),
    "crate_features": attr.string_list(),
    "rustc_flags": attr.string_list(),
}

_rust_toolchain_attrs = {
    "_rustc": attr.label(
        default = Label("//rust:rustc"),
        executable = True,
        cfg = "host",
        single_file = True,
    ),
    "_rustc_lib": attr.label(
        default = Label("//rust:rustc_lib"),
    ),
    "_rust_lib": attr.label(
        default = Label("//rust:rust_lib"),
    ),
    "_rustdoc": attr.label(
        default = Label("//rust:rustdoc"),
        executable = True,
        cfg = "host",
        single_file = True,
    ),
    "_crosstool": attr.label(
        default = Label("//tools/defaults:crosstool")
    ),
}

_rust_library_attrs = {
    "crate_type": attr.string(),
}

rust_library = rule(
    _rust_library_impl,
    attrs = dict(_rust_common_attrs.items() +
                 _rust_library_attrs.items() +
                 _rust_toolchain_attrs.items()),
    fragments = ["cpp"],
    outputs = {
        "rust_lib": "lib%{name}.rlib",
    },
)
"""Builds a Rust library crate.

Args:
  name: This name will also be used as the name of the library crate built by
    this rule.
  srcs: List of Rust `.rs` source files used to build the library.

    If `srcs` contains more than one file, then there must be a file either
    named `lib.rs`. Otherwise, `crate_root` must be set to the source file that
    is the root of the crate to be passed to rustc to build this crate.
  crate_root: The file that will be passed to `rustc` to be used for building
    this crate.

    If `crate_root` is not set, then this rule will look for a `lib.rs` file or
    the single file in `srcs` if `srcs` contains only one file.
  deps: List of other libraries to be linked to this library target.

    These can be either other `rust_library` targets or `cc_library` targets if
    linking a native library.
  data: List of files used by this rule at runtime.

    This attribute can be used to specify any data files that are embedded into
    the library, such as via the
    [`include_str!`](https://doc.rust-lang.org/std/macro.include_str!.html)
    macro.
  crate_features: List of features to enable for this crate.

    Features are defined in the code using the `#[cfg(feature = "foo")]`
    configuration option. The features listed here will be passed to `rustc`
    with `--cfg feature="${feature_name}"` flags.
  rustc_flags: List of compiler flags passed to `rustc`.

Example:
  Suppose you have the following directory structure for a simple Rust library
  crate:

  ```
  [workspace]/
      WORKSPACE
      hello_lib/
          BUILD
          src/
              greeter.rs
              lib.rs
  ```

  `hello_lib/src/greeter.rs`:

  ```rust
  pub struct Greeter {
      greeting: String,
  }

  impl Greeter {
      pub fn new(greeting: &str) -> Greeter {
          Greeter { greeting: greeting.to_string(), }
      }

      pub fn greet(&self, thing: &str) {
          println!("{} {}", &self.greeting, thing);
      }
  }
  ```

  `hello_lib/src/lib.rs`:


  ```rust
  pub mod greeter;
  ```

  `hello_lib/BUILD`:

  ```python
  package(default_visibility = ["//visibility:public"])

  load("@io_bazel_rules_rust//rust:rust.bzl", "rust_library")

  rust_library(
      name = "hello_lib",
      srcs = [
          "src/greeter.rs",
          "src/lib.rs",
      ],
  )
  ```

  Build the library:

  ```
  $ bazel build //hello_lib
  INFO: Found 1 target...
  Target //examples/rust/hello_lib:hello_lib up-to-date:
    bazel-bin/examples/rust/hello_lib/libhello_lib.rlib
  INFO: Elapsed time: 1.245s, Critical Path: 1.01s
  ```
"""

rust_binary = rule(
    _rust_binary_impl,
    attrs = dict(_rust_common_attrs.items() + _rust_toolchain_attrs.items()),
    executable = True,
    fragments = ["cpp"],
)
"""Builds a Rust binary crate.

Args:
  name: This name will also be used as the name of the binary crate built by
    this rule.
  srcs: List of Rust `.rs` source files used to build the library.

    If `srcs` contains more than one file, then there must be a file either
    named `main.rs`. Otherwise, `crate_root` must be set to the source file that
    is the root of the crate to be passed to rustc to build this crate.
  crate_root: The file that will be passed to `rustc` to be used for building
    this crate.

    If `crate_root` is not set, then this rule will look for a `bin.rs` file or
    the single file in `srcs` if `srcs` contains only one file.
  deps: List of other libraries to be linked to this library target.

    These must be `rust_library` targets.
  data: List of files used by this rule at runtime.

    This attribute can be used to specify any data files that are embedded into
    the library, such as via the
    [`include_str!`](https://doc.rust-lang.org/std/macro.include_str!.html)
    macro.
  crate_features: List of features to enable for this crate.

    Features are defined in the code using the `#[cfg(feature = "foo")]`
    configuration option. The features listed here will be passed to `rustc`
    with `--cfg feature="${feature_name}"` flags.
  rustc_flags: List of compiler flags passed to `rustc`.

Example:
  Suppose you have the following directory structure for a Rust project with a
  library crate, `hello_lib`, and a binary crate, `hello_world` that uses the
  `hello_lib` library:

  ```
  [workspace]/
      WORKSPACE
      hello_lib/
          BUILD
          src/
              lib.rs
      hello_world/
          BUILD
          src/
              main.rs
  ```

  `hello_lib/src/lib.rs`:

  ```rust
  pub struct Greeter {
      greeting: String,
  }

  impl Greeter {
      pub fn new(greeting: &str) -> Greeter {
          Greeter { greeting: greeting.to_string(), }
      }

      pub fn greet(&self, thing: &str) {
          println!("{} {}", &self.greeting, thing);
      }
  }
  ```

  `hello_lib/BUILD`:

  ```python
  package(default_visibility = ["//visibility:public"])

  load("@io_bazel_rules_rust//rust:rust.bzl", "rust_library")

  rust_library(
      name = "hello_lib",
      srcs = ["src/lib.rs"],
  )
  ```

  `hello_world/src/main.rs`:

  ```rust
  extern crate hello_lib;

  fn main() {
      let hello = hello_lib::Greeter::new("Hello");
      hello.greet("world");
  }
  ```

  `hello_world/BUILD`:

  ```python
  load("@io_bazel_rules_rust//rust:rust.bzl", "rust_binary")

  rust_binary(
      name = "hello_world",
      srcs = ["src/main.rs"],
      deps = ["//hello_lib"],
  )
  ```

  Build and run `hello_world`:

  ```
  $ bazel run //hello_world
  INFO: Found 1 target...
  Target //examples/rust/hello_world:hello_world up-to-date:
    bazel-bin/examples/rust/hello_world/hello_world
  INFO: Elapsed time: 1.308s, Critical Path: 1.22s

  INFO: Running command line: bazel-bin/examples/rust/hello_world/hello_world
  Hello world
  ```
"""

rust_test = rule(
    _rust_test_impl,
    attrs = dict(_rust_common_attrs.items() + _rust_toolchain_attrs.items()),
    executable = True,
    fragments = ["cpp"],
    test = True,
)
"""Builds a Rust test crate.

Args:
  name: This name will also be used as the name of the binary crate built by
    this rule.
  srcs: List of Rust `.rs` source files used to build the test.

    If `srcs` contains more than one file, then there must be a file either
    named `lib.rs`. Otherwise, `crate_root` must be set to the source file that
    is the root of the crate to be passed to rustc to build this crate.
  crate_root: The file that will be passed to `rustc` to be used for building
    this crate.

    If `crate_root` is not set, then this rule will look for a `lib.rs` file or
    the single file in `srcs` if `srcs` contains only one file.
  deps: List of other libraries to be linked to this library target.

    These must be `rust_library` targets.
  data: List of files used by this rule at runtime.

    This attribute can be used to specify any data files that are embedded into
    the library, such as via the
    [`include_str!`](https://doc.rust-lang.org/std/macro.include_str!.html)
    macro.
  crate_features: List of features to enable for this crate.

    Features are defined in the code using the `#[cfg(feature = "foo")]`
    configuration option. The features listed here will be passed to `rustc`
    with `--cfg feature="${feature_name}"` flags.
  rustc_flags: List of compiler flags passed to `rustc`.

Examples:
  Suppose you have the following directory structure for a Rust library crate
  with unit test code in the library sources:

  ```
  [workspace]/
      WORKSPACE
      hello_lib/
          BUILD
          src/
              lib.rs
  ```

  `hello_lib/src/lib.rs`:

  ```rust
  pub struct Greeter {
      greeting: String,
  }

  impl Greeter {
      pub fn new(greeting: &str) -> Greeter {
          Greeter { greeting: greeting.to_string(), }
      }

      pub fn greet(&self, thing: &str) {
          println!("{} {}", &self.greeting, thing);
      }
  }

  #[cfg(test)]
  mod tests {
      use super::Greeter;

      #[test]
      fn test_greeting() {
          let hello = Greeter::new("Hi");
          assert_eq!("Hi Rust", hello.greeting("Rust"));
      }
  }
  ```

  To build and run the tests, simply add a `rust_test` rule with no `srcs` and
  only depends on the `hello_lib` `rust_library` target:

  `hello_lib/BUILD`:

  ```python
  package(default_visibility = ["//visibility:public"])

  load("@io_bazel_rules_rust//rust:rust.bzl", "rust_library", "rust_test")

  rust_library(
      name = "hello_lib",
      srcs = ["src/lib.rs"],
  )

  rust_test(
      name = "hello_lib_test",
      deps = [":hello_lib"],
  )
  ```

  Run the test with `bazel build //hello_lib:hello_lib_test`.

  ### Example: `test` directory

  Integration tests that live in the [`tests` directory][int-tests], they are
  essentially built as separate crates. Suppose you have the following directory
  structure where `greeting.rs` is an integration test for the `hello_lib`
  library crate:

  [int-tests]: http://doc.rust-lang.org/book/testing.html#the-tests-directory

  ```
  [workspace]/
      WORKSPACE
      hello_lib/
          BUILD
          src/
              lib.rs
          tests/
              greeting.rs
  ```

  `hello_lib/tests/greeting.rs`:

  ```rust
  extern crate hello_lib;

  use hello_lib;

  #[test]
  fn test_greeting() {
      let hello = greeter::Greeter::new("Hello");
      assert_eq!("Hello world", hello.greeting("world"));
  }
  ```

  To build the `greeting.rs` integration test, simply add a `rust_test` target
  with `greeting.rs` in `srcs` and a dependency on the `hello_lib` target:

  `hello_lib/BUILD`:

  ```python
  package(default_visibility = ["//visibility:public"])

  load("@io_bazel_rules_rust//rust:rust.bzl", "rust_library", "rust_test")

  rust_library(
      name = "hello_lib",
      srcs = ["src/lib.rs"],
  )

  rust_test(
      name = "greeting_test",
      srcs = ["tests/greeting.rs"],
      deps = [":hello_lib"],
  )
  ```

  Run the test with `bazel build //hello_lib:hello_lib_test`.
"""

rust_bench_test = rule(
    _rust_bench_test_impl,
    attrs = dict(_rust_common_attrs.items() + _rust_toolchain_attrs.items()),
    executable = True,
    fragments = ["cpp"],
    test = True,
)
"""Builds a Rust benchmark test.

**Warning**: This rule is currently experimental. [Rust Benchmark
tests][rust-bench] require the `Bencher` interface in the unstable `libtest`
crate, which is behind the `test` unstable feature gate. As a result, using
this rule would require using a nightly binary release of Rust. A
`rust_toolchain` rule will be added in the [near future](#roadmap) to make it
easy to use a custom Rust toolchain, such as a nightly release.

[rust-bench]: https://doc.rust-lang.org/book/benchmark-tests.html

Args:
  name: This name will also be used as the name of the binary crate built by
    this rule.
  srcs: List of Rust `.rs` source files used to build the test.

    If `srcs` contains more than one file, then there must be a file either
    named `lib.rs`. Otherwise, `crate_root` must be set to the source file that
    is the root of the crate to be passed to rustc to build this crate.
  crate_root: The file that will be passed to `rustc` to be used for building
    this crate.

    If `crate_root` is not set, then this rule will look for a `lib.rs` file or
    the single file in `srcs` if `srcs` contains only one file.
  deps: List of other libraries to be linked to this library target.

    These must be `rust_library` targets.
  data: List of files used by this rule at runtime.

    This attribute can be used to specify any data files that are embedded into
    the library, such as via the
    [`include_str!`](https://doc.rust-lang.org/std/macro.include_str!.html)
    macro.
  crate_features: List of features to enable for this crate.

    Features are defined in the code using the `#[cfg(feature = "foo")]`
    configuration option. The features listed here will be passed to `rustc`
    with `--cfg feature="${feature_name}"` flags.
  rustc_flags: List of compiler flags passed to `rustc`.

Example:
  Suppose you have the following directory structure for a Rust project with a
  library crate, `fibonacci` with benchmarks under the `benches/` directory:

  ```
  [workspace]/
      WORKSPACE
      fibonacci/
          BUILD
          src/
              lib.rs
          benches/
              fibonacci_bench.rs
  ```

  `fibonacci/src/lib.rs`:

  ```rust
  pub fn fibonacci(n: u64) -> u64 {
      if n < 2 {
          return n;
      }
      let mut n1: u64 = 0;
      let mut n2: u64 = 1;
      for _ in 1..n {
          let sum = n1 + n2;
          n1 = n2;
          n2 = sum;
      }
      n2
  }
  ```

  `fibonacci/benches/fibonacci_bench.rs`:

  ```rust
  #![feature(test)]

  extern crate test;
  extern crate fibonacci;

  use test::Bencher;

  #[bench]
  fn bench_fibonacci(b: &mut Bencher) {
      b.iter(|| fibonacci::fibonacci(40));
  }
  ```

  To build the benchmark test, simply add a `rust_bench_test` target:

  `fibonacci/BUILD`:

  ```python
  package(default_visibility = ["//visibility:public"])

  load("@io_bazel_rules_rust//rust:rust.bzl", "rust_library", "rust_bench_test")

  rust_library(
      name = "fibonacci",
      srcs = ["src/lib.rs"],
  )

  rust_bench_test(
      name = "fibonacci_bench",
      srcs = ["benches/fibonacci_bench.rs"],
      deps = [":fibonacci"],
  )
  ```

  Run the benchmark test using: `bazel build //fibonacci:fibonacci_bench`.
"""

_rust_doc_common_attrs = {
    "dep": attr.label(mandatory = True),
}

_rust_doc_attrs = {
    "markdown_css": attr.label_list(allow_files = CSS_FILETYPE),
    "html_in_header": attr.label(allow_files = HTML_MD_FILETYPE),
    "html_before_content": attr.label(allow_files = HTML_MD_FILETYPE),
    "html_after_content": attr.label(allow_files = HTML_MD_FILETYPE),
}

rust_doc = rule(
    _rust_doc_impl,
    attrs = dict(_rust_doc_common_attrs.items() +
                 _rust_doc_attrs.items() +
                 _rust_toolchain_attrs.items()),
    outputs = {
        "rust_doc_zip": "%{name}-docs.zip",
    },
)
"""Generates code documentation.

Args:
  name: A unique name for this rule.
  dep: The label of the target to generate code documentation for.

    `rust_doc` can generate HTML code documentation for the source files of
    `rust_library` or `rust_binary` targets.
  markdown_css: CSS files to include via `<link>` in a rendered
    Markdown file.
  html_in_header: File to add to `<head>`.
  html_before_content: File to add in `<body>`, before content.
  html_after_content: File to add in `<body>`, after content.

Example:
  Suppose you have the following directory structure for a Rust library crate:

  ```
  [workspace]/
      WORKSPACE
      hello_lib/
          BUILD
          src/
              lib.rs
  ```

  To build [`rustdoc`][rustdoc] documentation for the `hello_lib` crate, define
  a `rust_doc` rule that depends on the the `hello_lib` `rust_library` target:

  [rustdoc]: https://doc.rust-lang.org/book/documentation.html

  ```python
  package(default_visibility = ["//visibility:public"])

  load("@io_bazel_rules_rust//rust:rust.bzl", "rust_library", "rust_doc")

  rust_library(
      name = "hello_lib",
      srcs = ["src/lib.rs"],
  )

  rust_doc(
      name = "hello_lib_doc",
      dep = ":hello_lib",
  )
  ```

  Running `bazel build //hello_lib:hello_lib_doc` will build a zip file containing
  the documentation for the `hello_lib` library crate generated by `rustdoc`.
"""

rust_doc_test = rule(
    _rust_doc_test_impl,
    attrs = dict(_rust_doc_common_attrs.items() +
                 _rust_toolchain_attrs.items()),
    executable = True,
    test = True,
)
"""Runs Rust documentation tests.

Args:
  name: A unique name for this rule.
  dep: The label of the target to run documentation tests for.

    `rust_doc_test` can run documentation tests for the source files of
    `rust_library` or `rust_binary` targets.

Example:
  Suppose you have the following directory structure for a Rust library crate:

  ```
  [workspace]/
      WORKSPACE
      hello_lib/
          BUILD
          src/
              lib.rs
  ```

  To run [documentation tests][doc-test] for the `hello_lib` crate, define a
  `rust_doc_test` target that depends on the `hello_lib` `rust_library` target:

  [doc-test]: https://doc.rust-lang.org/book/documentation.html#documentation-as-tests

  ```python
  package(default_visibility = ["//visibility:public"])

  load("@io_bazel_rules_rust//rust:rust.bzl", "rust_library", "rust_doc_test")

  rust_library(
      name = "hello_lib",
      srcs = ["src/lib.rs"],
  )

  rust_doc_test(
      name = "hello_lib_doc_test",
      dep = ":hello_lib",
  )
  ```

  Running `bazel test //hello_lib:hello_lib_doc_test` will run all documentation
  tests for the `hello_lib` library crate.
"""
