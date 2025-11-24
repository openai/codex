# @generated
def _generate_script_impl(ctx):
  script_file = ctx.actions.declare_file(ctx.label.name + ".bash")
  ctx.actions.write(output=script_file, is_executable=True, content="""
{0}
""".format(ctx.file.binary.short_path))
  return struct(
      files = depset([script_file]),
  )


generate_script = rule(
    _generate_script_impl,
    attrs = {
        "binary": attr.label(allow_files=True, single_file=True),
    },
)
