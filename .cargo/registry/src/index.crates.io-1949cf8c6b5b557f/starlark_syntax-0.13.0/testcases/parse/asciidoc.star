# @generated
def documentation_attributes():
  return [
    "toc",
    'newline="\\n"',
    'asterisk="&#42;"',
    'plus="&#43;"',
    'caret="&#94;"',
    'startsb="&#91;"',
    'endsb="&#93;"',
    'tilde="&#126;"',
    "last-update-label!",
    "source-highlighter=prettify",
    "stylesheet=DEFAULT",
    "linkcss=true",
    "prettifydir=.",
    # Just a placeholder, will be filled in asciidoctor java binary:
    "revnumber=%s",
  ]

def release_notes_attributes():
  return [
    'toc',
    'newline="\\n"',
    'asterisk="&#42;"',
    'plus="&#43;"',
    'caret="&#94;"',
    'startsb="&#91;"',
    'endsb="&#93;"',
    'tilde="&#126;"',
    'last-update-label!',
    'stylesheet=DEFAULT',
    'linkcss=true',
  ]

def _replace_macros_impl(ctx):
  cmd = [
    ctx.file._exe.path,
    '--suffix', ctx.attr.suffix,
    "-s", ctx.file.src.path,
    "-o", ctx.outputs.out.path,
  ]
  if ctx.attr.searchbox:
    cmd.append('--searchbox')
  else:
    cmd.append('--no-searchbox')
  ctx.action(
    inputs = [ctx.file._exe, ctx.file.src],
    outputs = [ctx.outputs.out],
    command = cmd,
    progress_message = "Replacing macros in %s" % ctx.file.src.short_path,
  )

_replace_macros = rule(
    attrs = {
        "_exe": attr.label(
            default = Label("//Documentation:replace_macros.py"),
            allow_single_file = True,
        ),
        "src": attr.label(
            mandatory = True,
            allow_single_file = [".txt"],
        ),
        "suffix": attr.string(mandatory = True),
        "searchbox": attr.bool(default = True),
        "out": attr.output(mandatory = True),
    },
    implementation = _replace_macros_impl,
)

def _generate_asciidoc_args(ctx):
  args = []
  if ctx.attr.backend:
    args.extend(["-b", ctx.attr.backend])
  revnumber = False
  for attribute in ctx.attr.attributes:
    if attribute.startswith("revnumber="):
      revnumber = True
    else:
      args.extend(["-a", attribute])
  if revnumber:
    args.extend([
      "--revnumber-file", ctx.file.version.path,
    ])
  for src in ctx.files.srcs:
    args.append(src.path)
  return args

def _invoke_replace_macros(name, src, suffix, searchbox):
  fn = src
  if fn.startswith(":"):
    fn = src[1:]

  _replace_macros(
    name = "macros_%s_%s" % (name, fn),
    src = src,
    out = fn + suffix,
    suffix = suffix,
    searchbox = searchbox,
  )

  return ":" + fn + suffix, fn.replace(".txt", ".html")

def _asciidoc_impl(ctx):
  args = [
    "--bazel",
    "--in-ext", ".txt" + ctx.attr.suffix,
    "--out-ext", ".html",
  ]
  args.extend(_generate_asciidoc_args(ctx))
  ctx.action(
    inputs = ctx.files.srcs + [ctx.executable._exe, ctx.file.version],
    outputs = ctx.outputs.outs,
    executable = ctx.executable._exe,
    arguments = args,
    progress_message = "Rendering asciidoctor files for %s" % ctx.label.name,
  )

_asciidoc_attrs = {
    "_exe": attr.label(
        default = Label("//lib/asciidoctor:asciidoc"),
        cfg = "host",
        allow_files = True,
        executable = True,
    ),
    "srcs": attr.label_list(
        mandatory = True,
        allow_files = True,
    ),
    "version": attr.label(
        default = Label("//:version.txt"),
        allow_single_file = True,
    ),
    "suffix": attr.string(mandatory = True),
    "backend": attr.string(),
    "attributes": attr.string_list(),
}

_asciidoc = rule(
    attrs = _asciidoc_attrs + {
        "outs": attr.output_list(mandatory = True),
    },
    implementation = _asciidoc_impl,
)

def _genasciidoc_htmlonly(
    name,
    srcs = [],
    attributes = [],
    backend = None,
    searchbox = True,
    **kwargs):
  SUFFIX = "." + name + "_macros"
  new_srcs = []
  outs = ["asciidoctor.css"]

  for src in srcs:
    new_src, html_name = _invoke_replace_macros(name, src, SUFFIX, searchbox)
    new_srcs.append(new_src)
    outs.append(html_name)

  _asciidoc(
    name = name + "_gen",
    srcs = new_srcs,
    suffix = SUFFIX,
    backend = backend,
    attributes = attributes,
    outs = outs,
  )

  native.filegroup(
    name = name,
    data = outs,
    **kwargs
  )

def genasciidoc(
    name,
    srcs = [],
    attributes = [],
    backend = None,
    searchbox = True,
    resources = True,
    **kwargs):
  SUFFIX = "_htmlonly"

  _genasciidoc_htmlonly(
    name = name + SUFFIX if resources else name,
    srcs = srcs,
    attributes = attributes,
    backend = backend,
    searchbox = searchbox,
    **kwargs
  )

  if resources:
    htmlonly = ":" + name + SUFFIX
    native.filegroup(
      name = name,
      srcs = [
        htmlonly,
        "//Documentation:resources",
      ],
      **kwargs
    )

def _asciidoc_html_zip_impl(ctx):
  args = [
    "--mktmp",
    "-z", ctx.outputs.out.path,
    "--in-ext", ".txt" + ctx.attr.suffix,
    "--out-ext", ".html",
  ]
  args.extend(_generate_asciidoc_args(ctx))
  ctx.action(
    inputs = ctx.files.srcs + [ctx.executable._exe, ctx.file.version],
    outputs = [ctx.outputs.out],
    executable = ctx.executable._exe,
    arguments = args,
    progress_message = "Rendering asciidoctor files for %s" % ctx.label.name,
  )

_asciidoc_html_zip = rule(
    attrs = _asciidoc_attrs,
    outputs = {
        "out": "%{name}.zip",
    },
    implementation = _asciidoc_html_zip_impl,
)

def _genasciidoc_htmlonly_zip(
    name,
    srcs = [],
    attributes = [],
    backend = None,
    searchbox = True,
    **kwargs):
  SUFFIX = "." + name + "_expn"
  new_srcs = []

  for src in srcs:
    new_src, _ = _invoke_replace_macros(name, src, SUFFIX, searchbox)
    new_srcs.append(new_src)

  _asciidoc_html_zip(
    name = name,
    srcs = new_srcs,
    suffix = SUFFIX,
    backend = backend,
    attributes = attributes,
  )

def _asciidoc_zip_impl(ctx):
  tmpdir = ctx.outputs.out.path + "_tmpdir"
  cmd = [
    "p=$PWD",
    "rm -rf %s" % tmpdir,
    "mkdir %s" % tmpdir,
    "unzip -q %s -d %s/%s/" % (ctx.file.src.path, tmpdir, ctx.attr.directory),
  ]
  for r in ctx.files.resources:
    if r.path == r.short_path:
      cmd.append("tar -cf- %s | tar -C %s -xf-" % (r.short_path, tmpdir))
    else:
      parent = r.path[:-len(r.short_path)]
      cmd.append(
        "tar -C %s -cf- %s | tar -C %s -xf-" % (parent, r.short_path, tmpdir))
  cmd.extend([
    "cd %s" % tmpdir,
    "zip -qr $p/%s *" % ctx.outputs.out.path,
  ])
  ctx.action(
    inputs = [ctx.file.src] + ctx.files.resources,
    outputs = [ctx.outputs.out],
    command = " && ".join(cmd),
    progress_message =
        "Generating asciidoctor zip file %s" % ctx.outputs.out.short_path,
  )

_asciidoc_zip = rule(
    attrs = {
        "src": attr.label(
            mandatory = True,
            allow_single_file = [".zip"],
        ),
        "resources": attr.label_list(
            mandatory = True,
            allow_files = True,
        ),
        "directory": attr.string(mandatory = True),
    },
    outputs = {
        "out": "%{name}.zip",
    },
    implementation = _asciidoc_zip_impl,
)

def genasciidoc_zip(
    name,
    srcs = [],
    attributes = [],
    directory = None,
    backend = None,
    searchbox = True,
    resources = True,
    **kwargs):
  SUFFIX = "_htmlonly"

  _genasciidoc_htmlonly_zip(
    name = name + SUFFIX if resources else name,
    srcs = srcs,
    attributes = attributes,
    backend = backend,
    searchbox = searchbox,
    **kwargs
  )

  if resources:
    htmlonly = ":" + name + SUFFIX
    _asciidoc_zip(
      name = name,
      src = htmlonly,
      resources = ["//Documentation:resources"],
      directory = directory,
    )
