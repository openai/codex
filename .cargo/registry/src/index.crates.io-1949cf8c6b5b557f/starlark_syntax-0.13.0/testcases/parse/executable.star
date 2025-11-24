# @generated
"""This example creates an executable rule.

An executable rule, like `cc_library`, can be run using 'bazel run'. It
can also be executed as part of the build.
"""

def _impl(ctx):
  # The implementation function must generate the file 'ctx.outputs.executable'.
  ctx.actions.write(
      output=ctx.outputs.executable,
      content="#!/bin/bash\necho Hello!",
      is_executable=True
  )
  # The executable output is added automatically to this target.

executable_rule = rule(
    implementation=_impl,
    executable=True
)
