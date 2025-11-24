# @generated
"""This example shows how to create custom (user defined) outputs for a rule.

This rule takes a list of output files from the user and writes content in
each of them.
"""

def _impl(ctx):
  # Access the custom outputs using ctx.outputs.<attribute name>.
  for output in ctx.outputs.outs:
    ctx.actions.write(
        output=output,
        content="I am " + output.short_path + "\n"
    )
  # The custom outputs are added automatically to this target.

rule_with_outputs = rule(
    implementation=_impl,
    attrs={
        "outs": attr.output_list()
    }
)
