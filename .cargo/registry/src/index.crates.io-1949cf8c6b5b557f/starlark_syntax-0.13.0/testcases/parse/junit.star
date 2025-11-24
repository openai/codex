# @generated
# Copyright (C) 2016 The Android Open Source Project
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

# Skylark rule to generate a Junit4 TestSuite
# Assumes srcs are all .java Test files
# Assumes junit4 is already added to deps by the user.

# See https://github.com/bazelbuild/bazel/issues/1017 for background.

_OUTPUT = """import org.junit.runners.Suite;
import org.junit.runner.RunWith;

@RunWith(Suite.class)
@Suite.SuiteClasses({%s})
public class %s {}
"""

_PREFIXES = ("org", "com", "edu")

def _SafeIndex(l, val):
    for i, v in enumerate(l):
        if val == v:
            return i
    return -1

def _AsClassName(fname):
    fname = [x.path for x in fname.files][0]
    toks = fname[:-5].split("/")
    findex = -1
    for s in _PREFIXES:
        findex = _SafeIndex(toks, s)
        if findex != -1:
            break
    if findex == -1:
        fail("%s does not contain any of %s",
                         fname, _PREFIXES)
    return ".".join(toks[findex:]) + ".class"

def _impl(ctx):
    classes = ",".join(
        [_AsClassName(x) for x in ctx.attr.srcs])
    ctx.file_action(output=ctx.outputs.out, content=_OUTPUT % (
            classes, ctx.attr.outname))

_GenSuite = rule(
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "outname": attr.string(),
    },
    outputs = {"out": "%{name}.java"},
    implementation = _impl,
)

def junit_tests(name, srcs, **kwargs):
    s_name = name + "TestSuite"
    _GenSuite(name = s_name,
              srcs = srcs,
              outname = s_name)
    native.java_test(name = name,
                     test_class = s_name,
                     srcs = srcs + [":"+s_name],
                     **kwargs)
