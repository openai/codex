# @generated
"""A basic go_proto_library.

Takes .proto as srcs and go_proto_library as deps
Note: can also work with a go_library(name=name,...)
      and a filegroup of .protos (name=name+"_protos",...)

A go_proto_library can then be a dependency of go_library or another go_proto_library.

Requires/Uses:
@io_bazel_rules_go for go_* macros/rules.

Does:
Generate protos using the open-source protoc and protoc-gen-go.
Handles transitive dependencies.
gRPC for service generation
Handles bazel-style names like 'foo_proto',
and also Go package-style like 'go_default_library'

Does not yet:
Gets confused if local protos use 'option go_package'

Usage:

In the BUILD file where protos are

load("@io_bazel_rules_go//proto:go_proto_library.bzl", "go_proto_library")

go_proto_library(
  name = "my_proto",
  srcs = ["my.proto"],
  deps = [
    ":other_proto",
    "@com_github_golang_protobuf//ptypes/duration:go_default_library",
  ],
)
"""

load("@io_bazel_rules_go//go:def.bzl", "go_library", "go_repository")

_DEFAULT_LIB = "go_default_library"  # matching go_library

_PROTOS_SUFFIX = "_protos"

_GO_GOOGLE_PROTOBUF = "go_google_protobuf"

_WELL_KNOWN_REPO = "@com_github_golang_protobuf//ptypes/"

def _collect_protos_import(ctx):
  """Collect the list of transitive protos and m_import_path.

  Paths of the form Mpath/to.proto=foo.com/bar specify a mapping into the global Go namespace.
  https://github.com/golang/protobuf#parameters

  Args:
    ctx: the standard bazel rule ctx object.

  Returns:
    (list of unique protos, list of m_import paths)
  """
  protos = depset()
  m_import_path = []
  for d in ctx.attr.deps:
    if not hasattr(d, "_protos"):
      # should be a raw filegroup then
      protos += list(d.files)
      continue
    protos += d._protos
    m_import_path.append(d._m_import_path)
  return list(protos), m_import_path

def _drop_external(path):
  """Drop leading '../' indicating an external dir of the form ../$some-repo.

  Non-generated external protos show up in a parallel directory.
  e.g. ptypes/any/any.proto is at ../com_github_golang_protobuf/ptypes/any/any.proto
  So this function detects and drops the 2 leading directories in this case.

  Args:
    path: short_path of a proto file

  Returns:
    A cleaned path.
  """
  if not path.startswith("../"):
    return path
  return "/".join(path.split("/")[2:])

def _check_bazel_style(ctx):
  """If the library name is not 'go_default_library', then we have to create an extra level of indirection."""
  if ctx.label.name == _DEFAULT_LIB + _PROTOS_SUFFIX:
    return ctx.outputs.outs, ""
  proto_outs = [
      ctx.new_file(
          ctx.configuration.bin_dir,
          s.basename[:-len(".proto")] + ".pb.go")
      for s in ctx.files.srcs
  ]
  for proto_out, ctx_out in zip(proto_outs, ctx.outputs.outs):
    ctx.action(
        inputs=[proto_out],
        outputs=[ctx_out],
        command="cp %s %s" % (proto_out.path, ctx_out.path),
        mnemonic="GoProtocGenCp")
  return proto_outs, "/" + ctx.label.name[:-len(_PROTOS_SUFFIX)]

def _go_proto_library_gen_impl(ctx):
  """Rule implementation that generates Go using protoc."""
  proto_outs, go_package_name = _check_bazel_style(ctx)

  go_prefix = ctx.attr.go_prefix.go_prefix
  if go_prefix and ctx.label.package and not go_prefix.endswith("/"):
    go_prefix = go_prefix + "/"
  source_go_package = "%s%s%s" % (go_prefix, ctx.label.package, go_package_name)

  m_imports = ["M%s=%s" % (_drop_external(f.short_path), source_go_package)
               for f in ctx.files.srcs]

  protos, mi = _collect_protos_import(ctx)
  m_import_path = ",".join(m_imports + mi)
  use_grpc = "plugins=grpc," if ctx.attr.grpc else ""

  # Create work dir, copy all protos there stripping of any external/bazel- prefixes.
  work_dir = ctx.outputs.outs[0].path + ".protoc"
  root_prefix = "/".join([".." for _ in work_dir.split("/")])
  cmds = ["set -e", "/bin/rm -rf %s; /bin/mkdir -p %s" % (work_dir, work_dir)]
  srcs = list(ctx.files.srcs)
  dirs = depset([s.short_path[:-1-len(s.basename)] for s in srcs + protos])
  cmds += ["mkdir -p %s/%s" % (work_dir, _drop_external(d)) for d in dirs if d]

  if ctx.attr.ignore_go_package_option:
    # Strip the "option go_package" line from the proto file before compiling,
    # c.f., https://github.com/bazelbuild/rules_go/issues/323
    #
    # NOTE: Using sed does not provide a perfect solution, build will break if
    # the go_package option splits multiple lines. Use with caution.
    cmds += ["sed '/^ *option  *go_package/d' %s > %s/%s" %
             (s.path, work_dir, _drop_external(s.short_path)) for s in srcs]
    cmds += ["cp %s %s/%s" % (s.path, work_dir, _drop_external(s.short_path))
             for s in protos]
  else:
    cmds += ["cp %s %s/%s" % (s.path, work_dir, _drop_external(s.short_path))
             for s in srcs + protos]
  cmds += ["cd %s" % work_dir,
           "%s/%s --go_out=%s%s:. %s" % (root_prefix, ctx.executable.protoc.path,
                                         use_grpc, m_import_path,
                                         " ".join([_drop_external(f.short_path) for f in srcs]))]
  cmds += ["cp %s %s/%s" % (_drop_external(p.short_path), root_prefix, p.path)
           for p in proto_outs]
  run = ctx.new_file(ctx.configuration.bin_dir, ctx.outputs.outs[0].basename + ".run")
  ctx.file_action(
      output = run,
      content = "\n".join(cmds),
      executable = True)

  ctx.action(
      inputs=srcs + protos + ctx.files.protoc_gen_go + [ctx.executable.protoc, run],
      outputs=proto_outs,
      progress_message="Generating into %s" % ctx.outputs.outs[0].dirname,
      mnemonic="GoProtocGen",
      env = {"PATH": root_prefix + "/" + ctx.files.protoc_gen_go[0].dirname +
             ":/bin:/usr/bin"},  # /bin/sed for linux, /usr/bin/sed for macos.
      executable=run)
  return struct(_protos=protos+srcs,
                _m_import_path=m_import_path)

_go_proto_library_gen = rule(
    attrs = {
        "deps": attr.label_list(),
        "srcs": attr.label_list(
            mandatory = True,
            allow_files = True,
        ),
        "grpc": attr.int(default = 0),
        "outs": attr.output_list(mandatory = True),
        "ignore_go_package_option": attr.int(default = 0),
        "protoc": attr.label(
            executable = True,
            single_file = True,
            allow_files = True,
            cfg = "host",
        ),
        "protoc_gen_go": attr.label(
            allow_files = True,
            cfg = "host",
        ),
        "_protos": attr.label_list(default = []),
        "go_prefix": attr.label(
            providers = ["go_prefix"],
            default = Label(
                "//:go_prefix",
                relative_to_caller_repository = True,
            ),
            allow_files = False,
            cfg = "host",
        ),
    },
    output_to_genfiles = True,
    implementation = _go_proto_library_gen_impl,
)

def _add_target_suffix(target, suffix):
  idx = target.find(":")
  if idx != -1:
    return target + suffix
  toks = target.split("/")
  return target + ":" + toks[-1] + suffix

def _well_known_proto_deps(deps, repo):
  for d in deps:
    if d.startswith(_WELL_KNOWN_REPO):
      return [repo + "//:" + _GO_GOOGLE_PROTOBUF]
  return []

def go_proto_library(name, srcs = None, deps = None,
                     has_services = 0,
                     testonly = 0, visibility = None,
                     ignore_go_package_option = 0,
                     protoc = "@com_github_google_protobuf//:protoc",
                     protoc_gen_go = "@com_github_golang_protobuf//protoc-gen-go",
                     x_net_context = "@org_golang_x_net//context:go_default_library",
                     google_grpc = "@org_golang_google_grpc//:go_default_library",
                     golang_protobuf = "@com_github_golang_protobuf//proto:go_default_library",
                     rules_go_repo_only_for_internal_use = "@io_bazel_rules_go",
                     **kwargs):
  """Macro which generates and compiles protobufs for Go.

  Args:
    name: name assigned to the underlying go_library,
          typically "foo_proto" for ["foo.proto"]
    srcs: a list of .proto source files, currently only 1 supported
    deps: a mixed list of either go_proto_libraries, or
          any go_library which has a companion
          filegroup(name=name+"_protos",...)
          which contains the protos which were used
    has_services: indicates the proto has gRPC services and deps
    testonly: mark as testonly
    visibility: visibility to use on underlying go_library
    ignore_go_package_option: if 1, ignore the "option go_package" statement in
                              the srcs proto files.
    protoc: override the default version of protoc.  Most users won't need this.
    protoc_gen_go: override the default version of protoc_gen_go.
                   Most users won't need this.
    x_net_context: override the default version of the context package.  Most
                   users won't need this.
    google_grpc: override the default version of grpc.  Most users won't need
                 this.
    golang_protobuf: override the default version of proto.  Most users won't
                     need this.
    rules_go_repo_only_for_internal_use: don't use this, only to allow
                                         internal tests to work.
    **kwargs: any other args which are passed through to the underlying go_library
  """
  if not name:
    fail("name is required", "name")
  if not srcs:
    fail("srcs required", "srcs")
  if not deps:
    deps = []
  # bazel-style
  outs = [name + "/" + s[:-len(".proto")] + ".pb.go"
          for s in srcs]
  if name == _DEFAULT_LIB:
    outs = [s[:-len(".proto")] + ".pb.go"
            for s in srcs]

  _go_proto_library_gen(
      name = name + _PROTOS_SUFFIX,
      srcs = srcs,
      deps = [_add_target_suffix(s, _PROTOS_SUFFIX)
              for s in deps] + _well_known_proto_deps(
                  deps, repo=rules_go_repo_only_for_internal_use),
      outs = outs,
      testonly = testonly,
      visibility = visibility,
      ignore_go_package_option = ignore_go_package_option,
      grpc = has_services,
      protoc = protoc,
      protoc_gen_go = protoc_gen_go,
  )
  grpc_deps = []
  if has_services:
    grpc_deps += [x_net_context, google_grpc]
  go_library(
      name = name,
      srcs = [":" + name + _PROTOS_SUFFIX],
      deps = deps + grpc_deps + [golang_protobuf],
      testonly = testonly,
      visibility = visibility,
      **kwargs
  )

def _well_known_import_key(name):
  return "%s%s:go_default_library" % (_WELL_KNOWN_REPO, name)

_well_known_imports = [
    "any",
    "duration",
    "empty",
    "struct",
    "timestamp",
    "wrappers",
]

# If you have well_known proto deps, rules_go will add a magic
# google/protobuf/ directory at the import root
def go_google_protobuf(name = _GO_GOOGLE_PROTOBUF):
  deps = [_add_target_suffix(_well_known_import_key(wk), _PROTOS_SUFFIX)
          for wk in _well_known_imports]
  outs = [wk + ".proto" for wk in _well_known_imports]

  native.genrule(
      name = name,
      srcs = deps,
      outs = ["google/protobuf/"+o for o in outs],
      cmd = "cp $(SRCS) $(@D)/google/protobuf/",
      visibility = ["//visibility:public"],
  )

def go_proto_repositories(shared = 1):
  """Add this to your WORKSPACE to pull in all of the needed dependencies."""
  print("DEPRECATED: go_proto_repositories is redundant and will be removed soon")
