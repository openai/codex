# @generated
"""Custom build macros for IntelliJ plugin handling.
"""

load(":intellij_plugin_debug_target.bzl", "intellij_plugin_debug_target")
load(":intellij_plugin.bzl", "intellij_plugin", "optional_plugin_xml")

def merged_plugin_xml(name, srcs, **kwargs):
  """Merges N plugin.xml files together."""
  merge_tool = "//build_defs:merge_xml"
  native.genrule(
      name = name,
      srcs = srcs,
      outs = [name + ".xml"],
      cmd = "./$(location {merge_tool}) $(SRCS) > $@".format(
          merge_tool=merge_tool,
      ),
      tools = [merge_tool],
      **kwargs)

def _optstr(name, value):
  return ("--" + name) if value else ""

def stamped_plugin_xml(name,
                       plugin_xml,
                       plugin_id = None,
                       plugin_name = None,
                       stamp_since_build=False,
                       stamp_until_build=False,
                       include_product_code_in_stamp=False,
                       version=None,
                       version_file=None,
                       changelog_file=None,
                       description_file=None,
                       vendor_file=None,
                       **kwargs):
  """Stamps a plugin xml file with the IJ build number.

  Args:
    name: name of this target
    plugin_xml: target plugin_xml to stamp
    plugin_id: the plugin ID to stamp
    plugin_name: the plugin name to stamp
    stamp_since_build: Add build number to idea-version since-build.
    stamp_until_build: Add build number to idea-version until-build.
    include_product_code_in_stamp: Whether the product code (eg. "IC")
        is included in since-build and until-build.
    version: A version number to stamp.
    version_file: A file with the version number to be included.
    changelog_file: A file with the changelog to be included.
    description_file: A file containing a plugin description to be included.
    vendor_file: A file containing the vendor info to be included.
    **kwargs: Any additional arguments to pass to the final target.
  """
  stamp_tool = "//build_defs:stamp_plugin_xml"

  api_version_txt_name = name + "_api_version"
  api_version_txt(
      name = api_version_txt_name,
  )

  args = [
      "./$(location {stamp_tool})",
      "--plugin_xml=$(location {plugin_xml})",
      "--api_version_txt=$(location {api_version_txt_name})",
      "{stamp_since_build}",
      "{stamp_until_build}",
      "{include_product_code_in_stamp}",
  ]
  srcs = [plugin_xml, api_version_txt_name]

  if version and version_file:
    fail("Cannot supply both version and version_file")

  if plugin_id:
    args.append("--plugin_id=%s" % plugin_id)

  if plugin_name:
    args.append("--plugin_name='%s'" % plugin_name)

  if version:
    args.append("--version='%s'" % version)

  if version_file:
    args.append("--version_file=$(location {version_file})")
    srcs.append(version_file)

  if changelog_file:
    args.append("--changelog_file=$(location {changelog_file})")
    srcs.append(changelog_file)

  if description_file:
    args.append("--description_file=$(location {description_file})")
    srcs.append(description_file)

  if vendor_file:
    args.append("--vendor_file=$(location {vendor_file})")
    srcs.append(vendor_file)

  cmd = " ".join(args).format(
      plugin_xml=plugin_xml,
      api_version_txt_name=api_version_txt_name,
      stamp_tool=stamp_tool,
      stamp_since_build=_optstr("stamp_since_build",
                                stamp_since_build),
      stamp_until_build=_optstr("stamp_until_build",
                                stamp_until_build),
      include_product_code_in_stamp=_optstr(
          "include_product_code_in_stamp",
          include_product_code_in_stamp),
      version_file=version_file,
      changelog_file=changelog_file,
      description_file=description_file,
      vendor_file=vendor_file,
  ) + "> $@"

  native.genrule(
      name = name,
      srcs = srcs,
      outs = [name + ".xml"],
      cmd = cmd,
      tools = [stamp_tool],
      **kwargs)

def product_build_txt(name, **kwargs):
  """Produces a product-build.txt file with the build number.

  Args:
    name: name of this target
    **kwargs: Any additional arguments to pass to the final target.
  """
  application_info_jar = "//intellij_platform_sdk:application_info_jar"
  application_info_name = "//intellij_platform_sdk:application_info_name"
  product_build_txt_tool = "//build_defs:product_build_txt"

  args = [
      "./$(location {product_build_txt_tool})",
      "--application_info_jar=$(location {application_info_jar})",
      "--application_info_name=$(location {application_info_name})",
  ]
  cmd = " ".join(args).format(
      application_info_jar=application_info_jar,
      application_info_name=application_info_name,
      product_build_txt_tool=product_build_txt_tool,
  ) + "> $@"
  native.genrule(
      name = name,
      srcs = [application_info_jar, application_info_name],
      outs = ["product-build.txt"],
      cmd = cmd,
      tools = [product_build_txt_tool],
      **kwargs)

def api_version_txt(name, **kwargs):
  """Produces an api_version.txt file with the api version, including the product code.

  Args:
    name: name of this target
    **kwargs: Any additional arguments to pass to the final target.
  """
  application_info_jar = "//intellij_platform_sdk:application_info_jar"
  application_info_name = "//intellij_platform_sdk:application_info_name"
  api_version_txt_tool = "//build_defs:api_version_txt"

  args = [
      "./$(location {api_version_txt_tool})",
      "--application_info_jar=$(location {application_info_jar})",
      "--application_info_name=$(location {application_info_name})",
  ]
  cmd = " ".join(args).format(
      application_info_jar=application_info_jar,
      application_info_name=application_info_name,
      api_version_txt_tool=api_version_txt_tool,
  ) + "> $@"
  native.genrule(
      name = name,
      srcs = [application_info_jar, application_info_name],
      outs = [name + ".txt"],
      cmd = cmd,
      tools = [api_version_txt_tool],
      **kwargs)

def repackaged_jar(name, deps, rules, **kwargs):
  """Repackages classes in a jar, to avoid collisions in the classpath.

  Args:
    name: the name of this target
    deps: The dependencies repackage
    rules: the rules to apply in the repackaging
        Do not repackage:
        - com.google.net.** because that has JNI files which use
          FindClass(JNIEnv *, const char *) with hard-coded native string
          literals that jarjar doesn't rewrite.
        - com.google.errorprone packages (rewriting will throw off blaze build).
    **kwargs: Any additional arguments to pass to the final target.
  """
  java_binary_name = name + "_orig"
  out = name + ".jar"
  native.java_binary(
      name = java_binary_name,
      create_executable = 0,
      stamp = 0,
      runtime_deps = deps)
  _repackaged_jar(name, java_binary_name, out, rules, **kwargs)

def _repackaged_jar(name, src_rule, out, rules, **kwargs):
  """Repackages classes in a jar, to avoid collisions in the classpath."""
  repackage_tool = "@jarjar//jar"
  deploy_jar = "{src_rule}_deploy.jar".format(src_rule=src_rule)
  script_lines = []
  script_lines.append("echo >> /tmp/repackaged_rule.txt")
  for rule in rules:
    script_lines.append("echo 'rule {rule}' >> /tmp/repackaged_rule.txt;".format(rule=rule))
  script_lines.append(" ".join([
      "$(location {repackage_tool})",
      "process /tmp/repackaged_rule.txt",
      "$(location {deploy_jar})",
      "$@",
  ]).format(
      repackage_tool = repackage_tool,
      deploy_jar = deploy_jar,
  ))
  genrule_name = name + "_repackaged"
  native.genrule(
      name = genrule_name,
      srcs = [deploy_jar],
      outs = [out],
      tools = [repackage_tool],
      cmd = "\n".join(script_lines),
  )
  native.java_import(
      name = name,
      jars = [out],
      **kwargs)

def beta_gensignature(name, srcs, stable, stable_version, beta_version):
  if stable_version == beta_version:
    native.alias(name = name, actual = stable)
  else:
    native.gensignature(name = name, srcs = srcs)
