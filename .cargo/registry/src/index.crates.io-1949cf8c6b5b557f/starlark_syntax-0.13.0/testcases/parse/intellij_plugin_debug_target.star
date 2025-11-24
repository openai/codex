# @generated
"""IntelliJ plugin debug target rule used for debugging IntelliJ plugins.

Creates a plugin target debuggable from IntelliJ. Any files in
the 'deps' attribute are deployed to the plugin sandbox.

Any files are stripped of their prefix and installed into
<sandbox>/plugins. If you need structure, first put the files
into a pkgfilegroup. The files will be installed relative to the
'plugins' directory if present in the pkgfilegroup prefix.

intellij_plugin_debug_targets can be nested.

pkgfilegroup(
  name = "foo_files",
  srcs = [
    ":my_plugin_jar",
    ":my_additional_plugin_files",
  ],
  prefix = "plugins/foo/lib",
)

intellij_plugin_debug_target(
  name = "my_debug_target",
  deps = [
    ":my_jar",
  ],
)

"""

SUFFIX = ".intellij-plugin-debug-target-deploy-info"

def _trim_start(path, prefix):
  return path[len(prefix):] if path.startswith(prefix) else path

def _pkgfilegroup_deploy_file(ctx, f):
  strip_prefix = ctx.rule.attr.strip_prefix
  prefix = ctx.rule.attr.prefix
  if strip_prefix == ".":
    stripped_relative_path = f.basename
  elif strip_prefix.startswith("/"):
    stripped_relative_path = _trim_start(f.short_path, strip_prefix[1:])
  else:
    stripped_relative_path = _trim_start(f.short_path, PACKAGE_NAME)
    stripped_relative_path = _trim_start(stripped_relative_path, strip_prefix)
  stripped_relative_path = _trim_start(stripped_relative_path, "/")

  # If there's a 'plugins' directory, make destination relative to that
  plugini = prefix.find("plugins/")
  plugins_prefix = prefix[plugini + len("plugins/"):] if plugini >= 0 else prefix

  # If the install location is still absolute, fail
  if plugins_prefix.startswith("/"):
    fail("Cannot compute plugins-relative install directory for pkgfilegroup")

  dest = plugins_prefix + "/" + stripped_relative_path if plugins_prefix else stripped_relative_path
  return struct(
      src = f,
      deploy_location = dest,
  )

def _flat_deploy_file(f):
  return struct(
      src = f,
      deploy_location = f.basename,
  )

def _intellij_plugin_debug_target_aspect_impl(target, ctx):
  aspect_intellij_plugin_deploy_info = None

  if ctx.rule.kind == "intellij_plugin_debug_target":
    aspect_intellij_plugin_deploy_info = target.intellij_plugin_deploy_info
  elif ctx.rule.kind == "pkgfilegroup":
    aspect_intellij_plugin_deploy_info = struct(
        deploy_files = [_pkgfilegroup_deploy_file(ctx, f) for f in target.files],
    )
  else:
    aspect_intellij_plugin_deploy_info = struct(
        deploy_files = [_flat_deploy_file(f) for f in target.files],
    )

  return struct(
      files = target.files,
      aspect_intellij_plugin_deploy_info = aspect_intellij_plugin_deploy_info,
  )

_intellij_plugin_debug_target_aspect = aspect(
    implementation = _intellij_plugin_debug_target_aspect_impl,
)

def _build_deploy_info_file(deploy_file):
  return struct(
      execution_path = deploy_file.src.path,
      deploy_location = deploy_file.deploy_location,
  )

def _intellij_plugin_debug_target_impl(ctx):
  files = set_which_is_banned()
  deploy_files = []
  for target in ctx.attr.deps:
    files = files | target.files
    deploy_files.extend(target.aspect_intellij_plugin_deploy_info.deploy_files)
  deploy_info = struct(
      deploy_files = [_build_deploy_info_file(f) for f in deploy_files]
  )
  output = ctx.new_file(ctx.label.name + SUFFIX)
  ctx.file_action(output, deploy_info.to_proto())

  # We've already consumed any dependent intellij_plugin_debug_targets into our own,
  # do not build or report these
  files = set_which_is_banned([f for f in files if not f.path.endswith(SUFFIX)])
  files = files | set_which_is_banned([output])

  return struct(
      files = files,
      intellij_plugin_deploy_info = struct(
          deploy_files = deploy_files,
      )
  )

intellij_plugin_debug_target = rule(
    implementation = _intellij_plugin_debug_target_impl,
    attrs = {
        "deps": attr.label_list(aspects = [_intellij_plugin_debug_target_aspect]),
    },
)
