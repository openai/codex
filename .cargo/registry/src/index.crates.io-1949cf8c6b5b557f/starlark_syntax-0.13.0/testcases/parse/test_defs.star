# @generated
"""Custom rule for creating IntelliJ plugin tests.
"""

load(
    "//build_defs:build_defs.bzl",
    "api_version_txt",
)

def _generate_test_suite_impl(ctx):
  """Generates a JUnit4 test suite pulling in all the referenced classes.

  Args:
    ctx: the rule context
  """
  suite_class_name = ctx.label.name
  lines = []
  lines.append("package %s;" % ctx.attr.test_package_root)
  lines.append("")
  test_srcs = _get_test_srcs(ctx.attr.srcs)
  test_classes = [_get_test_class(test_src, ctx.attr.test_package_root) for test_src in test_srcs]
  class_rules = ctx.attr.class_rules
  if (class_rules):
    lines.append("import org.junit.ClassRule;")
  lines.append("import org.junit.runner.RunWith;")
  lines.append("import org.junit.runners.Suite;")
  lines.append("")
  for test_class in test_classes:
    lines.append("import %s;" % test_class)
  lines.append("")
  lines.append("@RunWith(Suite.class)")
  lines.append("@Suite.SuiteClasses({")
  for test_class in test_classes:
    lines.append("    %s.class," % test_class.split(".")[-1])
  lines.append("})")
  lines.append("public class %s {" % suite_class_name)
  lines.append("")

  i = 1
  for class_rule in class_rules:
    lines.append("@ClassRule")
    lines.append("public static %s setupRule_%d = new %s();" % (class_rule, i, class_rule))
    i += 1

  lines.append("}")

  contents = "\n".join(lines)
  ctx.file_action(
      output = ctx.outputs.out,
      content = contents,
  )

_generate_test_suite = rule(
    implementation = _generate_test_suite_impl,
    attrs = {
        # srcs for the test classes included in the suite (only keep those ending in Test.java)
        "srcs": attr.label_list(allow_files=True, mandatory=True),
        # the package string of the output test suite.
        "test_package_root": attr.string(mandatory=True),
        # optional list of classes to instantiate as a @ClassRule in the test suite.
        "class_rules": attr.string_list()
    },
    outputs={"out": "%{name}.java"},
)

def intellij_unit_test_suite(name, srcs, test_package_root, **kwargs):
  """Creates a java_test rule comprising all valid test classes in the specified srcs.

  Only classes ending in "Test.java" will be recognized.

  Args:
    name: name of this rule.
    srcs: the test classes.
    test_package_root: only tests under this package root will be run.
    **kwargs: Any other args to be passed to the java_test.
  """
  suite_class_name = name + "TestSuite"
  suite_class = test_package_root + "." + suite_class_name
  _generate_test_suite(
      name = suite_class_name,
      srcs = srcs,
      test_package_root = test_package_root,
  )
  native.java_test(
      name = name,
      srcs = srcs + [suite_class_name],
      test_class = suite_class,
      **kwargs)

def intellij_integration_test_suite(
    name,
    srcs,
    test_package_root,
    deps,
    size="medium",
    shard_count=None,
    jvm_flags = [],
    runtime_deps = [],
    platform_prefix="Idea",
    required_plugins=None,
    **kwargs):
  """Creates a java_test rule comprising all valid test classes in the specified srcs.

  Only classes ending in "Test.java" will be recognized.

  All test classes must be located in the blaze package calling this function.

  Args:
    name: name of this rule.
    srcs: the test classes.
    test_package_root: only tests under this package root will be run.
    deps: the required deps.
    size: the test size.
    shard_count: the number of shards to use.
    jvm_flags: extra flags to be passed to the test vm.
    runtime_deps: the required runtime_deps.
    platform_prefix: Specifies the JetBrains product these tests are run against. Examples are
        'Idea' (IJ CE), 'idea' (IJ UE), 'CLion', 'AndroidStudio'. See
        com.intellij.util.PlatformUtils for other options.
    required_plugins: optional comma-separated list of plugin IDs. Integration tests will fail if
        these plugins aren't loaded at runtime.
    **kwargs: Any other args to be passed to the java_test.
  """
  suite_class_name = name + "TestSuite"
  suite_class = test_package_root + "." + suite_class_name
  _generate_test_suite(
      name = suite_class_name,
      srcs = srcs,
      test_package_root = test_package_root,
      class_rules = ["com.google.idea.testing.BlazeTestSystemPropertiesRule"],
  )

  api_version_txt_name = name + "_api_version"
  api_version_txt(name = api_version_txt_name)
  data = kwargs.pop("data", [])
  data.append(api_version_txt_name)

  deps = list(deps)
  deps.extend([
      "//testing:lib",
  ])
  runtime_deps = list(runtime_deps)
  runtime_deps.extend([
      "//intellij_platform_sdk:bundled_plugins",
      "//third_party:jpda-jdi",
  ])

  jvm_flags = list(jvm_flags)
  jvm_flags.extend([
      "-Didea.classpath.index.enabled=false",
      "-Djava.awt.headless=true",
      "-Didea.platform.prefix=" + platform_prefix,
      "-Dblaze.idea.api.version.file=$(location %s)" % api_version_txt_name
  ])

  if required_plugins:
    jvm_flags.append("-Didea.required.plugins.id=" + required_plugins)

  native.java_test(
      name = name,
      size = size,
      srcs = srcs + [suite_class_name],
      data = data,
      shard_count = shard_count,
      jvm_flags = jvm_flags,
      test_class = suite_class,
      runtime_deps = runtime_deps,
      deps = deps,
      **kwargs
  )

def _get_test_class(test_src, test_package_root):
  """Returns the package string of the test class, beginning with the given root."""
  test_path = test_src.short_path
  temp = test_path[:-len(".java")]
  temp = temp.replace("/", ".")
  i = temp.rfind(test_package_root)
  if i < 0:
    fail("Test source '%s' not under package root '%s'" % (test_path, test_package_root))
  test_class = temp[i:]
  return test_class

def _get_test_srcs(targets):
  """Returns all files of the given targets that end with Test.java."""
  files = set_which_is_banned()
  for target in targets:
    files += target.files
  return [f for f in files if f.basename.endswith("Test.java")]
