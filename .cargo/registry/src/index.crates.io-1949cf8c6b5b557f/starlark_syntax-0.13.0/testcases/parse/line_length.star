# @generated
"""Test rule that fails if a source file has too long lines."""

def _check_file(f, columns):
  """Return shell commands for testing file 'f'."""
  # We write information to stdout. It will show up in logs, so that the user
  # knows what happened if the test fails.
  return """
echo Testing that {file} has at most {columns} columns...
grep -E '^.{{{columns}}}' {path} && err=1
echo
""".format(columns=columns, path=f.path, file=f.short_path)

def _impl(ctx):
  script = "\n".join(
      ["err=0"] +
      [_check_file(f, ctx.attr.columns) for f in ctx.files.srcs] +
      ["exit $err"])

  # Write the file, it is executed by 'bazel test'.
  ctx.actions.write(
      output=ctx.outputs.executable,
      content=script)

  # To ensure the files needed by the script are available, we put them in
  # the runfiles.
  runfiles = ctx.runfiles(files=ctx.files.srcs)
  return [DefaultInfo(runfiles=runfiles)]

line_length_test = rule(
    implementation=_impl,
    attrs={
        "columns": attr.int(default = 100),
        "srcs": attr.label_list(allow_files=True)
    },
    test=True)
