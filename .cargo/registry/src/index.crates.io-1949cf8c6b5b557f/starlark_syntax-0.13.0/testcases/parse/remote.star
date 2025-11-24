# @generated
def _test_chdir_remote_impl(ctx):
  ctx.file("WORKSPACE", """workspace("test_chdir_remote")""")
  ctx.file("BUILD.bazel", "")
  for f in ["BUILD.bazel", "data_test.go", "data.txt"]:
    input = Label("@io_bazel_rules_go//tests/test_chdir:{}".format(f))
    ctx.template("sub/" + f, input)

_test_chdir_remote = repository_rule(
    implementation = _test_chdir_remote_impl,
    attrs = {},
)

def test_chdir_remote():
  _test_chdir_remote(name="test_chdir_remote")