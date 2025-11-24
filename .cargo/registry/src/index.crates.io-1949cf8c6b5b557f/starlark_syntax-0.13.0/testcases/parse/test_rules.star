# @generated
"""Utilities for testing bazel."""
#
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

### First, trivial tests that either always pass, always fail,
### or sometimes pass depending on a trivial computation.

def success_target(ctx, msg):
  """Return a success for an analysis test.

  The test rule must have an executable output.

  Args:
    ctx: the Bazel rule context
    msg: an informative message to display

  Returns:
    a suitable rule implementation struct(),
    with actions that always succeed at execution time.
  """
  exe = ctx.outputs.executable
  dat = ctx.new_file(ctx.genfiles_dir, exe, ".dat")
  ctx.file_action(
      output=dat,
      content=msg)
  ctx.file_action(
      output=exe,
      content="cat " + dat.path + " ; echo",
      executable=True)
  return struct(runfiles=ctx.runfiles([exe, dat]))

def _successful_test_impl(ctx):
  return success_target(ctx, ctx.attr.msg)

successful_test = rule(
    attrs = {"msg": attr.string(mandatory = True)},
    executable = True,
    test = True,
    implementation = _successful_test_impl,
)

def failure_target(ctx, msg):
  """Return a failure for an analysis test.

  The test rule must have an executable output.

  Args:
    ctx: the Bazel rule context
    msg: an informative message to display

  Returns:
    a suitable rule implementation struct(),
    with actions that always fail at execution time.
  """
  ### fail(msg) ### <--- This would fail at analysis time.
  exe = ctx.outputs.executable
  dat = ctx.new_file(ctx.genfiles_dir, exe, ".dat")
  ctx.file_action(
      output=dat,
      content=msg)
  ctx.file_action(
      output=exe,
      content="(cat " + dat.short_path + " ; echo ) >&2 ; exit 1",
      executable=True)
  return struct(runfiles=ctx.runfiles([exe, dat]))

def _failed_test_impl(ctx):
  return failure_target(ctx, ctx.attr.msg)

failed_test = rule(
    attrs = {"msg": attr.string(mandatory = True)},
    executable = True,
    test = True,
    implementation = _failed_test_impl,
)

### Second, general purpose utilities

def assert_(condition, string="assertion failed", *args):
  """Trivial assertion mechanism.

  Args:
    condition: a generalized boolean expected to be true
    string: a format string for the error message should the assertion fail
    *args: format arguments for the error message should the assertion fail

  Returns:
    None.

  Raises:
    an error if the condition isn't true.
  """

  if not condition:
    fail(string % args)

def strip_prefix(prefix, string):
  assert_(string.startswith(prefix),
          "%s does not start with %s", string, prefix)
  return string[len(prefix):len(string)]

def expectation_description(expect=None, expect_failure=None):
  """Turn expectation of result or error into a string."""
  if expect_failure:
    return "failure " + str(expect_failure)
  else:
    return "result " + repr(expect)

def check_results(result, failure, expect, expect_failure):
  """See if actual computation results match expectations.

  Args:
    result: the result returned by the test if it ran to completion
    failure: the failure message caught while testing, if any
    expect: the expected result for a successful test, if no failure expected
    expect_failure: the expected failure message for the test, if any

  Returns:
    a pair (tuple) of a boolean (true if success) and a message (string).
  """
  wanted = expectation_description(expect, expect_failure)
  found = expectation_description(result, failure)
  if wanted == found:
    return (True, "successfully computed " + wanted)
  else:
    return (False, "expect " + wanted + " but found " + found)

def load_results(name, result=None, failure=None,
                 expect=None, expect_failure=None):
  """issue load-time results of a test.

  Args:
    name: the name of the Bazel rule at load time.
    result: the result returned by the test if it ran to completion
    failure: the failure message caught while testing, if any
    expect: the expected result for a successful test, if no failure expected
    expect_failure: the expected failure message for the test, if any

  Returns:
    None, after issuing a rule that will succeed at execution time if
    expectations were met.
  """
  (is_success, msg) = check_results(result, failure, expect, expect_failure)
  this_test = successful_test if is_success else failed_test
  return this_test(name=name, msg=msg)

def analysis_results(ctx, result=None, failure=None,
                     expect=None, expect_failure=None):
  """issue analysis-time results of a test.

  Args:
    ctx: the Bazel rule context
    result: the result returned by the test if it ran to completion
    failure: the failure message caught while testing, if any
    expect: the expected result for a successful test, if no failure expected
    expect_failure: the expected failure message for the test, if any

  Returns:
    a suitable rule implementation struct(),
    with actions that succeed at execution time if expectation were met,
    or fail at execution time if they didn't.
  """
  (is_success, msg) = check_results(result, failure, expect, expect_failure)
  this_test = success_target if is_success else failure_target
  return this_test(ctx, msg)

### Simple tests

def _rule_test_impl(ctx):
  """check that a rule generates the desired outputs and providers."""
  rule_ = ctx.attr.rule
  rule_name = str(rule_.label)
  exe = ctx.outputs.executable
  if ctx.attr.generates:
    # Generate the proper prefix to remove from generated files.
    prefix_parts = []

    if rule_.label.workspace_root:
      # Create a prefix that is correctly relative to the output of this rule.
      prefix_parts = ["..", strip_prefix("external/", rule_.label.workspace_root)]

    if rule_.label.package:
      prefix_parts.append(rule_.label.package)

    prefix = "/".join(prefix_parts)

    if prefix:
      # If the prefix isn't empty, it needs a trailing slash.
      prefix = prefix + "/"

    # TODO(bazel-team): Use set_which_is_banned() instead of sorted() once
    # set comparison is implemented.
    # TODO(bazel-team): Use a better way to determine if two paths refer to
    # the same file.
    generates = sorted(ctx.attr.generates)
    generated = sorted([strip_prefix(prefix, f.short_path)
                        for f in rule_.files])
    if generates != generated:
      fail("rule %s generates %s not %s"
           % (rule_name, repr(generated), repr(generates)))
  provides = ctx.attr.provides
  if provides:
    files = []
    commands = []
    for k in provides.keys():
      if hasattr(rule_, k):
        v = repr(getattr(rule_, k))
      else:
        fail(("rule %s doesn't provide attribute %s. "
              + "Its list of attributes is: %s")
             % (rule_name, k, dir(rule_)))
      file_ = ctx.new_file(ctx.genfiles_dir, exe, "." + k)
      files += [file_]
      regexp = provides[k]
      commands += [
          "if ! grep %s %s ; then echo 'bad %s:' ; cat %s ; echo ; exit 1 ; fi"
          % (repr(regexp), file_.short_path, k, file_.short_path)]
      ctx.file_action(output=file_, content=v)
    script = "\n".join(commands + ["true"])
    ctx.file_action(output=exe, content=script, executable=True)
    return struct(runfiles=ctx.runfiles([exe] + files))
  else:
    return success_target(ctx, "success")

rule_test = rule(
    attrs = {
        "rule": attr.label(mandatory = True),
        "generates": attr.string_list(),
        "provides": attr.string_dict(),
    },
    executable = True,
    test = True,
    implementation = _rule_test_impl,
)

def _file_test_impl(ctx):
  """check that a file has a given content."""
  exe = ctx.outputs.executable
  file_ = ctx.file.file
  content = ctx.attr.content
  regexp = ctx.attr.regexp
  matches = ctx.attr.matches
  if bool(content) == bool(regexp):
    fail("Must specify one and only one of content or regexp")
  if content and matches != -1:
    fail("matches only makes sense with regexp")
  if content:
    dat = ctx.new_file(ctx.genfiles_dir, exe, ".dat")
    ctx.file_action(
        output=dat,
        content=content)
    ctx.file_action(
        output=exe,
        content="diff -u %s %s" % (dat.short_path, file_.short_path),
        executable=True)
    return struct(runfiles=ctx.runfiles([exe, dat, file_]))
  if matches != -1:
    script = "[ %s == $(grep -c %s %s) ]" % (
        matches, repr(regexp), file_.short_path)
  else:
    script = "grep %s %s" % (repr(regexp), file_.short_path)
  ctx.file_action(
      output=exe,
      content=script,
      executable=True)
  return struct(runfiles=ctx.runfiles([exe, file_]))

file_test = rule(
    attrs = {
        "file": attr.label(
            mandatory = True,
            allow_files = True,
            single_file = True,
        ),
        "content": attr.string(default = ""),
        "regexp": attr.string(default = ""),
        "matches": attr.int(default = -1),
    },
    executable = True,
    test = True,
    implementation = _file_test_impl,
)
