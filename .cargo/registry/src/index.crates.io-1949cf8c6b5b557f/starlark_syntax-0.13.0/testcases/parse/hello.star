# @generated
# Label of the template file to use.
_TEMPLATE = "//expand_template:hello.cc"

def _hello_impl(ctx):
  ctx.actions.expand_template(
      template=ctx.file._template,
      output=ctx.outputs.source_file,
      substitutions={
          "{FIRSTNAME}": ctx.attr.firstname
      })

hello = rule(
    implementation=_hello_impl,
    attrs={
        "firstname": attr.string(mandatory=True),
        "_template": attr.label(
            default=Label(_TEMPLATE), allow_files=True, single_file=True),
    },
    outputs={"source_file": "%{name}.cc"},
)
