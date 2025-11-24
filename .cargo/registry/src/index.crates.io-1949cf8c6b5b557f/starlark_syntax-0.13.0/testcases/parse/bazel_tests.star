# @generated
load("@io_bazel_rules_go//go/private:go_repository.bzl", "env_execute")
load("@io_bazel_rules_go//go/private:common.bzl", "declare_file")

# _bazelrc is the bazel.rc file that sets the default options for tests
_bazelrc = """
build --verbose_failures
build --sandbox_debug
build --test_output=errors
build --spawn_strategy=standalone
build --genrule_strategy=standalone

test --test_strategy=standalone

build:isolate --
build:fetch --fetch=True
"""

# _basic_workspace is the content appended to all test workspace files
# it contains the calls required to make the go rules work
_basic_workspace = """
load("@io_bazel_rules_go//go:def.bzl", "go_rules_dependencies", "go_register_toolchains")
load("@io_bazel_rules_go//proto:def.bzl", "proto_register_toolchains")
go_rules_dependencies()
proto_register_toolchains()
"""

# _bazel_test_script_template is the template for the bazel invocation script
_bazel_test_script_template = """
echo running in {work_dir}
unset TEST_TMPDIR
RULES_GO_OUTPUT={output}

mkdir -p {work_dir}
mkdir -p {cache_dir}
cp -f {workspace} {work_dir}/WORKSPACE
cp -f {build} {work_dir}/BUILD.bazel
cd {work_dir}

{bazel} --bazelrc {bazelrc} --nomaster_blazerc {command}  --experimental_repository_cache={cache_dir} --config {config} {args} {target} >& bazel-output.txt
result=$?

{check}

if (( $result != 0 )); then
  echo "Bazel output: $(<bazel-output.txt)"
fi
exit $result
"""

# _env_build_template is the template for the bazel test environment repository build file
_env_build_template = """
load("@io_bazel_rules_go//tests:bazel_tests.bzl", "bazel_test_settings")
bazel_test_settings(
  name = "settings",
  bazel = "{bazel}",
  exec_root = "{exec_root}",
  scratch_dir = "{scratch_dir}",
  visibility = ["//visibility:public"],
)
filegroup(
  name = "bazelrc",
  srcs = ["test.bazelrc"],
  visibility = ["//visibility:public"],
)
"""

CURRENT_VERSION = "current"

def _bazel_test_script_impl(ctx):
  script_file = declare_file(ctx, ext=".bash")

  if ctx.attr.go_version == CURRENT_VERSION:
    register = 'go_register_toolchains()\n'
  elif ctx.attr.go_version != None:
    register = 'go_register_toolchains(go_version="{}")\n'.format(ctx.attr.go_version)

  workspace_content = 'workspace(name = "bazel_test")\n\n'
  for ext in ctx.attr.externals:
    root = ext.label.workspace_root
    _,_,name = ext.label.workspace_root.rpartition("/")
    workspace_content += 'local_repository(name="{name}", path="{exec_root}/{root}")\n'.format(
        name = name,
        root = root,
        exec_root = ctx.attr._settings.exec_root,
    )
  if ctx.attr.workspace:
    workspace_content += ctx.attr.workspace
  else:
    workspace_content += _basic_workspace.format()
    workspace_content += register

  workspace_file = declare_file(ctx, path="WORKSPACE.in")
  ctx.actions.write(workspace_file, workspace_content)
  build_file = declare_file(ctx, path="BUILD.in")
  ctx.actions.write(build_file, ctx.attr.build)

  targets = ["@" + ctx.workspace_name + "//" + ctx.label.package + t if t.startswith(":") else t for t in ctx.attr.targets]
  output = "external/" + ctx.workspace_name + "/" + ctx.label.package
  script_content = _bazel_test_script_template.format(
      bazelrc = ctx.attr._settings.exec_root+"/"+ctx.file._bazelrc.path,
      config = ctx.attr.config,
      command = ctx.attr.command,
      args = " ".join(ctx.attr.args),
      target = " ".join(targets),
      check = ctx.attr.check,
      workspace = workspace_file.short_path,
      build = build_file.short_path,
      output = output,
      bazel = ctx.attr._settings.bazel,
      work_dir = ctx.attr._settings.scratch_dir + "/" + ctx.attr.config,
      cache_dir = ctx.attr._settings.scratch_dir + "/cache",
  )
  ctx.actions.write(output=script_file, is_executable=True, content=script_content)
  return struct(
      files = depset([script_file]),
      runfiles = ctx.runfiles([workspace_file, build_file])
  )


_bazel_test_script = rule(
    _bazel_test_script_impl,
    attrs = {
        "command": attr.string(mandatory=True, values=["build", "test", "coverage", "run"]),
        "args": attr.string_list(default=[]),
        "targets": attr.string_list(mandatory=True),
        "externals": attr.label_list(allow_files=True),
        "go_version": attr.string(default=CURRENT_VERSION),
        "workspace": attr.string(),
        "build": attr.string(),
        "check": attr.string(),
        "config": attr.string(default="isolate"),
        "_bazelrc": attr.label(allow_files=True, single_file=True, default="@bazel_test//:bazelrc"),
        "_settings": attr.label(default = Label("@bazel_test//:settings")),
    },
)

def bazel_test(name, command = None, args=None, targets = None, go_version = None, tags=[], externals=[], workspace="", build="", check="", config=None):
  script_name = name+"_script"
  externals = externals + [
      "@io_bazel_rules_go//:AUTHORS",
      "@local_config_cc//:cc_wrapper",
  ]
  if go_version == None or go_version == CURRENT_VERSION:
      externals.append("@go_sdk//:packages.txt")

  _bazel_test_script(
      name = script_name,
      command = command,
      args = args,
      targets = targets,
      externals = externals,
      go_version = go_version,
      workspace = workspace,
      build = build,
      check = check,
      config = config,
  )
  native.sh_test(
      name = name,
      size = "large",
      timeout = "moderate",
      srcs = [":" + script_name],
      tags = ["local", "bazel", "exclusive"] + tags,
      data = [
          "@bazel_test//:bazelrc",
          "//tests:rules_go_deps",
          "//go/tools/gazelle/gazelle",
      ],
  )

def _md5_sum_impl(ctx):
  out = declare_file(ctx, ext=".md5")
  arguments = ctx.actions.args()
  arguments.add(["-output", out.path])
  arguments.add(ctx.files.srcs)
  ctx.actions.run(
      inputs = ctx.files.srcs,
      outputs = [out],
      mnemonic = "GoMd5sum",
      executable = ctx.file._md5sum,
      arguments = [arguments],
  )
  return struct(files=depset([out]))

md5_sum = rule(
    _md5_sum_impl,
    attrs = {
        "srcs": attr.label_list(allow_files=True),
        "_md5sum":  attr.label(allow_files=True, single_file=True, default=Label("@io_bazel_rules_go//go/tools/builders:md5sum")),
    },
)

def _test_environment_impl(ctx):
  # Find bazel
  bazel = ""
  if "BAZEL" in ctx.os.environ:
    bazel = ctx.os.environ["BAZEL"]
  elif "BAZEL_VERSION" in ctx.os.environ:
    home = ctx.os.environ["HOME"]
    bazel = home + "/.bazel/{0}/bin/bazel".format(ctx.os.environ["BAZEL_VERSION"])
  if bazel == "" or not ctx.path(bazel).exists:
    bazel = ctx.which("bazel")

  # Get a temporary directory to use as our scratch workspace
  result = env_execute(ctx, ["mktemp", "-d"])
  if result.return_code:
    fail("failed to create temporary directory for bazel tests: {}".format(result.stderr))
  scratch_dir = result.stdout.strip()

  # Work out where we are running so we can find externals
  exec_root, _, _ = str(ctx.path(".")).rpartition("/external/")

  # build the basic environment
  ctx.file("WORKSPACE", 'workspace(name = "{}")'.format(ctx.name))
  ctx.file("BUILD.bazel", _env_build_template.format(
      bazel = bazel,
      exec_root = exec_root,
      scratch_dir = scratch_dir,
  ))
  ctx.file("test.bazelrc", content=_bazelrc)

_test_environment = repository_rule(
    implementation = _test_environment_impl,
    attrs = {},
    environ = [
        "BAZEL",
        "BAZEL_VERSION",
        "HOME",
    ],
)

def _bazel_test_settings_impl(ctx):
  return struct(
      bazel = ctx.attr.bazel,
      exec_root = ctx.attr.exec_root,
      scratch_dir = ctx.attr.scratch_dir,
  )

bazel_test_settings = rule(
    _bazel_test_settings_impl,
    attrs = {
        "bazel": attr.string(mandatory = True),
        "exec_root": attr.string(mandatory = True),
        "scratch_dir": attr.string(mandatory = True),
    },
)

def test_environment():
  _test_environment(name="bazel_test")
