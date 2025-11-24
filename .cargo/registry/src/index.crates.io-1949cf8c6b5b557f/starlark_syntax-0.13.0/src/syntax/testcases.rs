/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use crate::syntax::grammar_tests;

macro_rules! testcases_parse {
    ($($x:expr)*) => {
        &[
            $(
                (
                    $x,
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/testcases/parse/",
                        $x,
                    ))
                )
            ),*
         ]
    }
}

const TESTCASE_FILES: &[(&str, &str)] = testcases_parse!(
    // A list of all files from testcases/parse, minus README.md
    // If you add additional parse tests, make sure to update this list.
    // If Rust adds list_directory! as a macro, remove this list.
    "action.star"
    "alias_rules.star"
    "android_sdk_repository_template.star"
    "archive.star"
    "asciidoc.star"
    "asm.star"
    "aspect.star"
    "bazel_hash_dict.star"
    "bazel_integration_test.star"
    "bazel_java_integration_test.star"
    "bazel_py_integration_test.star"
    "bazel_tests.star"
    "binary.star"
    "bindata.star"
    "bower_archives.star"
    "bower_components.star"
    "build.star"
    "build_defs.star"
    "bundle.star"
    "bzl.star"
    "cc.star"
    "cc_configure.star"
    "cgo.star"
    "classpath.star"
    "cm.star"
    "common.star"
    "compile.star"
    "compiler.star"
    "config.star"
    "container.star"
    "cover.star"
    "csharp.star"
    "def.star"
    "dicts.star"
    "docker.star"
    "docker_base.star"
    "docker_pull.star"
    "docker_repository.star"
    "dockerfile_build.star"
    "dummy_toolchain.star"
    "e4b_aspect.star"
    "eclipse.star"
    "eclipse_platform.star"
    "embed_data.star"
    "embedded_tools.star"
    "empty.star"
    "executable.star"
    "extension.star"
    "external_plugin_deps.star"
    "files_equal_test.star"
    "filetype.star"
    "flatten.star"
    "flavours.star"
    "foo.star"
    "gazelle.star"
    "generate_test.star"
    "generate_workspace.star"
    "genproto.star"
    "genrule2.star"
    "git.star"
    "git_repositories.star"
    "go.star"
    "go_proto_library.star"
    "go_repository.star"
    "go_toolchain.star"
    "guava.star"
    "gwt.star"
    "hash.star"
    "hello.star"
    "http.star"
    "image.star"
    "import.star"
    "info.star"
    "intellij_plugin.star"
    "intellij_plugin_debug_target.star"
    "java.star"
    "java_rules_skylark.star"
    "javadoc.star"
    "jekyll.star"
    "jenkins.star"
    "jenkins_base.star"
    "jenkins_docker_build.star"
    "jenkins_job.star"
    "jenkins_node.star"
    "jenkins_nodes.star"
    "jetty.star"
    "jgit.star"
    "jobs.star"
    "js.star"
    "junit.star"
    "label.star"
    "layers.star"
    "lib_cc_configure.star"
    "library.star"
    "license.star"
    "line_length.star"
    "lines_sorted_test.star"
    "link.star"
    "list.star"
    "list_source_repository.star"
    "load.star"
    "maven.star"
    "maven_jar.star"
    "maven_rules.star"
    "mode.star"
    "oci.star"
    "osx_archs.star"
    "osx_cc_configure.star"
    "pack.star"
    "package.star"
    "passwd.star"
    "path.star"
    "paths.star"
    "pkg.star"
    "pkg_war.star"
    "plugin.star"
    "plugins.star"
    "popular_repos.star"
    "prefix.star"
    "printer.star"
    "prolog.star"
    "proto_alias.star"
    "protobuf.star"
    "providers.star"
    "pull.star"
    "push-all.star"
    "push.star"
    "python.star"
    "redirects.star"
    "remote.star"
    "repositories.star"
    "repository_tools.star"
    "rpm.star"
    "rust.star"
    "self_extract_binary.star"
    "serialize.star"
    "sets.star"
    "shared.star"
    "shell.star"
    "single_output_test.star"
    "site.star"
    "source.star"
    "stdlib.star"
    "structs.star"
    "templates.star"
    "test.star"
    "test_defs.star"
    "test_rules.star"
    "tests.star"
    "toolchain.star"
    "toolchain_utils.star"
    "toolchains.star"
    "transitive_maven_jar.star"
    "unix_cc_configure.star"
    "utilities.star"
    "vars.star"
    "version.star"
    "vet.star"
    "win_rules.star"
    "windows_cc_configure.star"
    "with-defaults.star"
    "with-tag.star"
    "workspace.star"
    "wrappers.star"
    "xcode_configure.star"
    "xcode_version_flag.star"
    "zip.star"
);

#[test]
fn parsing_testcases() {
    for (_, content) in TESTCASE_FILES {
        // Worth doing the lex and parse as lex checks for additional invariants
        grammar_tests::parse(content);
    }
}
