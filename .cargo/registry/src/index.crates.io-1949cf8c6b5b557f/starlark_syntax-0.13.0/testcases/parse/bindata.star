# @generated
load("@io_bazel_rules_go//go/private:go_repository.bzl", "go_repository")
load("@io_bazel_rules_go//go/private:common.bzl", "declare_file")

def _bindata_impl(ctx):
  out = declare_file(ctx, ext=".go")
  arguments = ctx.actions.args()
  arguments.add([
      "-o", out.path,
      "-pkg", ctx.attr.package,
      "-prefix", ctx.label.package,
  ])
  if not ctx.attr.compress:
    arguments.add("-nocompress")
  if not ctx.attr.metadata:
    arguments.add("-nometadata")
  arguments.add(ctx.files.srcs)
  ctx.actions.run(
    inputs = ctx.files.srcs,
    outputs = [out],
    mnemonic = "GoBindata",
    executable = ctx.file._bindata,
    arguments = [arguments],
  )
  return [
    DefaultInfo(
      files = depset([out])
    )
  ]

bindata = rule(
    _bindata_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True, cfg = "data"),
        "package": attr.string(mandatory=True),
        "compress": attr.bool(default=True),
        "metadata": attr.bool(default=False),
        "_bindata":  attr.label(allow_files=True, single_file=True, default=Label("@com_github_jteeuwen_go_bindata//go-bindata:go-bindata")),
    },
)
