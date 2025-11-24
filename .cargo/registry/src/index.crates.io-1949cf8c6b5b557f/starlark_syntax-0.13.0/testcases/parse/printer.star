# @generated
"""Example of a rule that accesses its attributes."""

def _impl(ctx):
  # Print debug information about the target.
  print("Target {} has {} deps".format(ctx.label, len(ctx.attr.deps)))

  # For each target in deps, print its label and files.
  for i, d in enumerate(ctx.attr.deps):
    print(" {}. label = {}".format(i+1, d.label))
    # A label can represent any number of files (possibly 0).
    print("    files = " + str([f.path for f in d.files]))

printer = rule(
    implementation=_impl,
    attrs={
        # Do not declare "name": It is added automatically.
        "number": attr.int(default = 1),
        "deps": attr.label_list(allow_files=True),
    })
