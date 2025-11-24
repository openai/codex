# @generated
"""IntelliJ plugin target rule.

Creates a plugin jar with the given plugin xml and any
optional plugin xmls.

To provide optional plugin xmls, use the 'optional_plugin_xml'
rule. These will be renamed, put in the META-INF directory,
and the main plugin xml stamped with optional plugin dependencies
that point to the correct META-INF optional plugin xmls.

optional_plugin_xml(
  name = "optional_python_xml",
  plugin_xml = "my_optional_python_plugin.xml",
  module = "com.idea.python.module.id",
)

intellij_plugin(
  name = "my_plugin",
  plugin_xml = ["my_plugin.xml"],
  optional_plugin_xmls = [":optional_python_xml"],
  deps = [
    ":code_deps",
  ],
)

"""

optional_plugin_xml_provider = provider()

def _optional_plugin_xml_impl(ctx):
  attr = ctx.attr
  optional_plugin_xmls = []
  if ctx.file.plugin_xml:
    optional_plugin_xmls.append(struct(
        plugin_xml = ctx.file.plugin_xml,
        module = attr.module,
    ))
  return struct(
      optional_plugin_xml_data = optional_plugin_xml_provider(
          optional_plugin_xmls = optional_plugin_xmls,
      ),
  )

optional_plugin_xml = rule(
    implementation = _optional_plugin_xml_impl,
    attrs = {
        "plugin_xml": attr.label(mandatory=True, allow_single_file=[".xml"]),
        "module": attr.string(mandatory=True),
    },
)

def _merge_optional_plugin_xmls(ctx):
  # Collect optional plugin xmls
  module_to_xmls = {}
  for target in ctx.attr.optional_plugin_xmls:
    if not hasattr(target, "optional_plugin_xml_data"):
      fail("optional_plugin_xmls only accepts optional_plugin_xml targets")
    for xml in target.optional_plugin_xml_data.optional_plugin_xmls:
      module = xml.module
      plugin_xmls = module_to_xmls.setdefault(module, [])
      plugin_xmls.append(xml.plugin_xml)

  # Merge xmls with the same module dependency
  module_to_merged_xmls = {}
  for module, plugin_xmls in module_to_xmls.items():
    merged_name = "merged_xml_for_" + module + "_" + ctx.label.name + ".xml"
    merged_file = ctx.new_file(merged_name)
    ctx.action(
        executable = ctx.executable._merge_xml_binary,
        arguments = ["--output", merged_file.path] + [plugin_xml.path for plugin_xml in plugin_xmls],
        inputs = list(plugin_xmls),
        outputs = [merged_file],
        progress_message = "Merging optional xmls",
        mnemonic = "MergeOptionalXmls",
    )
    module_to_merged_xmls[module] = merged_file
  return module_to_merged_xmls

def _add_optional_dependencies_to_plugin_xml(ctx, modules):
  input_plugin_xml_file = ctx.file.plugin_xml
  if not modules:
    return input_plugin_xml_file

  # Add optional dependencies into the plugin xml
  args = []
  final_plugin_xml_file = ctx.new_file("final_plugin_xml_" + ctx.label.name + ".xml")
  args.extend(["--plugin_xml", input_plugin_xml_file.path])
  args.extend(["--output", final_plugin_xml_file.path])
  for module in modules:
    args.append(module)
    args.append(_filename_for_module_dependency(module))
  ctx.action(
      executable = ctx.executable._append_optional_xml_elements,
      arguments = args,
      inputs = [input_plugin_xml_file],
      outputs = [final_plugin_xml_file],
      progress_message = "Adding optional dependencies to final plugin xml",
      mnemonic = "AddModuleDependencies",
  )
  return final_plugin_xml_file

def _only_file(target):
  return list(target.files)[0]

def _filename_for_module_dependency(module):
  """A unique filename for the optional xml dependency for a given module."""
  return "optional-" + module + ".xml"

def _package_meta_inf_files(ctx, final_plugin_xml_file, module_to_merged_xmls):
  jar_name = ctx.attr.jar_name
  jar_file = ctx.new_file(jar_name)

  args = []
  args.extend(["--deploy_jar", ctx.file.deploy_jar.path])
  args.extend(["--output", jar_file.path])
  args.extend([final_plugin_xml_file.path, "plugin.xml"])
  for module, merged_xml in module_to_merged_xmls.items():
    args.append(merged_xml.path)
    args.append(_filename_for_module_dependency(module))
  ctx.action(
      executable = ctx.executable._package_meta_inf_files,
      arguments = args,
      inputs = [ctx.file.deploy_jar, final_plugin_xml_file] + module_to_merged_xmls.values(),
      outputs = [jar_file],
      mnemonic = "PackagePluginJar",
      progress_message = "Packaging plugin jar",
  )
  return jar_file

def _intellij_plugin_jar_impl(ctx):
  module_to_merged_xmls = _merge_optional_plugin_xmls(ctx)
  final_plugin_xml_file = _add_optional_dependencies_to_plugin_xml(ctx, module_to_merged_xmls.keys())
  jar_file = _package_meta_inf_files(ctx, final_plugin_xml_file, module_to_merged_xmls)
  files = set_which_is_banned([jar_file])
  return struct(
      files = files,
  )

_intellij_plugin_jar = rule(
    implementation = _intellij_plugin_jar_impl,
    attrs = {
        "deploy_jar": attr.label(mandatory=True, allow_single_file=[".jar"]),
        "plugin_xml": attr.label(mandatory=True, allow_single_file=[".xml"]),
        "optional_plugin_xmls": attr.label_list(),
        "jar_name": attr.string(mandatory=True),
        "_merge_xml_binary": attr.label(
            default = Label("//build_defs:merge_xml"),
            executable = True,
            cfg = "host",
        ),
        "_append_optional_xml_elements": attr.label(
            default = Label("//build_defs:append_optional_xml_elements"),
            executable = True,
            cfg = "host",
        ),
        "_package_meta_inf_files": attr.label(
            default = Label("//build_defs:package_meta_inf_files"),
            executable = True,
            cfg = "host",
        ),
    },
)

def intellij_plugin(name, deps, plugin_xml, optional_plugin_xmls=[], jar_name=None, **kwargs):
  """Creates an intellij plugin from the given deps and plugin.xml.

  Args:
    name: The name of the target
    deps: Any java dependencies rolled up into the plugin jar.
    plugin_xml: An xml file to be placed in META-INF/plugin.jar
    optional_plugin_xmls: A list of optional_plugin_xml targets.
    jar_name: The name of the final plugin jar, or <name>.jar if None
    **kwargs: Any further arguments to be passed to the final target
  """
  binary_name = name + "_binary"
  deploy_jar = binary_name + "_deploy.jar"
  native.java_binary(
      name = binary_name,
      runtime_deps = deps,
      create_executable = 0,
  )
  jar_target_name =  name + "_intellij_plugin_jar"
  _intellij_plugin_jar(
      name = jar_target_name,
      deploy_jar = deploy_jar,
      jar_name = jar_name or (name + ".jar"),
      plugin_xml = plugin_xml,
      optional_plugin_xmls = optional_plugin_xmls,
  )
  # included (with tag) as a hack so that IJwB can recognize this is an intellij plugin
  native.java_import(
      name = name,
      jars = [jar_target_name],
      tags = ["intellij-plugin"],
      **kwargs)

def _append_optional_dependencies(name, plugin_xml, module_to_merged_xml):
  """Appends optional dependency xml elements to plugin xml."""
  append_elements_tool = "//build_defs:append_optional_xml_elements"
  args = [
      "./$(location {append_elements_tool})",
      "--plugin_xml=$(location {plugin_xml})",
      "--optional_xml_files={merged_optional_xml_files}",
  ]
  dictionary = {k: _filename_for_module_dependency(k) for k in module_to_merged_xml.keys()}
  cmd = " ".join(args).format(
      append_elements_tool=append_elements_tool,
      plugin_xml=plugin_xml,
      merged_optional_xml_files='"%s"' % str(dictionary).replace('"', '\\"'),
  ) + "> $@"

  srcs = module_to_merged_xml.values() + [plugin_xml]

  native.genrule(
      name = name,
      srcs = srcs,
      outs = [name + ".xml"],
      cmd = cmd,
      tools = [append_elements_tool],
  )
