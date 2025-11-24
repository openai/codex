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

# Java integration test framework for using Bazel
load(":common.bzl", "BAZEL_VERSIONS")
load(":repositories.bzl", "bazel_binaries")

def _index(lst, el):
  return lst.index(el) if el in lst else -1


def _java_package():
  # Adaptation of the java class finding library from Bazel.
  path = native.package_name()
  segments = path.split("/")
  roots = [segments.index(i)
           for i in ["java", "javatests", "src"]
           if i in segments]
  if not len(roots):
    return ".".join(segments)
  idx = min(roots)
  is_src = segments[idx] == "src"
  check_mvn_idx = idx if is_src else -1
  if idx == 0 or is_src:
    # Check for a nested root directory.
    end_segments = segments[idx + 1:-1]
    src_segment = end_segments.index("src") if "src" in end_segments else -1
    if is_src:
      end_segments_idx = [end_segments.index(i)
                          for i in ["java", "javatests", "src"]
                          if i in end_segments]
      if end_segments_idx:
        src_segment = min(end_segments_idx)
    if src_segment >= 0:
        next = end_segments[src_segment+1]
        if next in ["com", "org", "net"]:
          # Check for common first element of java package, to avoid false
          # positives.
          idx += src_segment + 1
        elif next in ["main", "test"]:
          # Also accept maven style src/(main|test)/(java|resources).
          check_mvn_idx = idx + src_segment + 1
  # Check for (main|test)/(java|resources) after /src/.
  if check_mvn_idx >= 0 and check_mvn_idx < len(segments) - 2:
    if segments[check_mvn_idx + 1] in ["main", "test"] and segments[check_mvn_idx + 2] in ["java", "resources"]:
      idx = check_mvn_idx + 2
  if idx < 0:
    return ".".join(segments)
  return ".".join(segments[idx+1:])


def bazel_java_integration_test(name, srcs=[], deps=None, runtime_deps=[],
                                jvm_flags=[], test_class=None,
                                versions=BAZEL_VERSIONS, **kwargs):
  """A wrapper around java_test that create several java tests, one per version
     of Bazel.

     Args:
       versions: list of version of bazel to create a test for. Each test
         will be named `<name>/bazel<version>`.
       See java_test for the other arguments.
  """
  if not test_class:
    test_class = "%s.%s" % (_java_package(), name)
  add_deps = [
    str(Label("//java/build/bazel/tests/integration")),
    "@org_hamcrest_core//jar",
    "@org_junit//jar",
  ]
  if srcs:
    deps = (deps or []) + add_deps
  else:
    runtime_deps = runtime_deps + add_deps
  for version in versions:
    native.java_test(
        name = "%s/bazel%s" % (name, version),
        jvm_flags = ["-Dbazel.version=" + version],
        srcs = srcs,
        test_class = test_class,
        deps = deps,
        runtime_deps = runtime_deps,
        **kwargs)
  native.test_suite(
        name = name,
        tests = [":%s/bazel%s" % (name, version) for version in versions])

def bazel_java_integration_test_deps(versions = BAZEL_VERSIONS):
  bazel_binaries(versions)

  # TODO(dmarting): Use http_file and relies on a mirror instead of maven_jar
  native.maven_jar(
      name = "com_google_guava",
      artifact = "com.google.guava:guava:jar:21.0",
  )

  native.maven_jar(
      name = "org_hamcrest_core",
      artifact = "org.hamcrest:hamcrest-core:jar:1.3",
  )

  native.maven_jar(
      name = "org_junit",
      artifact = "junit:junit:jar:4.11",
  )
