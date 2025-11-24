# @generated
GERRIT = "GERRIT:"

GERRIT_API = "GERRIT_API:"

MAVEN_CENTRAL = "MAVEN_CENTRAL:"

MAVEN_LOCAL = "MAVEN_LOCAL:"

def _maven_release(ctx, parts):
  """induce jar and url name from maven coordinates."""
  if len(parts) not in [3, 4]:
    fail('%s:\nexpected id="groupId:artifactId:version[:classifier]"'
         % ctx.attr.artifact)
  if len(parts) == 4:
    group, artifact, version, classifier = parts
    file_version = version + '-' + classifier
  else:
    group, artifact, version = parts
    file_version = version

  jar = artifact.lower() + '-' + file_version
  url = '/'.join([
    ctx.attr.repository,
    group.replace('.', '/'),
    artifact,
    version,
    artifact + '-' + file_version])

  return jar, url

# Creates a struct containing the different parts of an artifact's FQN
def _create_coordinates(fully_qualified_name):
  parts = fully_qualified_name.split(":")
  packaging = None
  classifier = None

  if len(parts) == 3:
    group_id, artifact_id, version = parts
  elif len(parts) == 4:
    group_id, artifact_id, version, packaging = parts
  elif len(parts) == 5:
    group_id, artifact_id, version, packaging, classifier = parts
  else:
    fail("Invalid fully qualified name for artifact: %s" % fully_qualified_name)

  return struct(
      fully_qualified_name = fully_qualified_name,
      group_id = group_id,
      artifact_id = artifact_id,
      packaging = packaging,
      classifier = classifier,
      version = version,
  )

def _format_deps(attr, deps):
  formatted_deps = ""
  if deps:
    if len(deps) == 1:
      formatted_deps += "%s = [\'%s\']," % (attr, deps[0])
    else:
      formatted_deps += "%s = [\n" % attr
      for dep in deps:
        formatted_deps += "        \'%s\',\n" % dep
      formatted_deps += "    ],"
  return formatted_deps

def _generate_build_file(ctx, binjar, srcjar):
  srcjar_attr = ""
  if srcjar:
    srcjar_attr = 'srcjar = "%s",' % srcjar
  contents = """
# DO NOT EDIT: automatically generated BUILD file for maven_jar rule {rule_name}
package(default_visibility = ['//visibility:public'])
java_import(
    name = 'jar',
    jars = ['{binjar}'],
    {srcjar_attr}
    {deps}
    {exports}
)
java_import(
    name = 'neverlink',
    jars = ['{binjar}'],
    neverlink = 1,
    {deps}
    {exports}
)
\n""".format(srcjar_attr = srcjar_attr,
              rule_name = ctx.name,
              binjar = binjar,
              deps = _format_deps("deps", ctx.attr.deps),
              exports = _format_deps("exports", ctx.attr.exports))
  if srcjar:
    contents += """
java_import(
    name = 'src',
    jars = ['{srcjar}'],
)
""".format(srcjar = srcjar)
  ctx.file('%s/BUILD' % ctx.path("jar"), contents, False)

def _maven_jar_impl(ctx):
  """rule to download a Maven archive."""
  coordinates = _create_coordinates(ctx.attr.artifact)

  name = ctx.name
  sha1 = ctx.attr.sha1

  parts = ctx.attr.artifact.split(':')
  # TODO(davido): Only releases for now, implement handling snapshots
  jar, url = _maven_release(ctx, parts)

  binjar = jar + '.jar'
  binjar_path = ctx.path('/'.join(['jar', binjar]))
  binurl = url + '.jar'

  python = ctx.which("python")
  script = ctx.path(ctx.attr._download_script)

  args = [python, script, "-o", binjar_path, "-u", binurl]
  if ctx.attr.sha1:
    args.extend(["-v", sha1])
  if ctx.attr.unsign:
    args.append('--unsign')
  for x in ctx.attr.exclude:
    args.extend(['-x', x])

  out = ctx.execute(args)

  if out.return_code:
    fail("failed %s: %s" % (' '.join(args), out.stderr))

  srcjar = None
  if ctx.attr.src_sha1 or ctx.attr.attach_source:
    srcjar = jar + '-src.jar'
    srcurl = url + '-sources.jar'
    srcjar_path = ctx.path('jar/' + srcjar)
    args = [python, script, "-o", srcjar_path, "-u", srcurl]
    if ctx.attr.src_sha1:
      args.extend(['-v', ctx.attr.src_sha1])
    out = ctx.execute(args)
    if out.return_code:
      fail("failed %s: %s" % (args, out.stderr))

  _generate_build_file(ctx, binjar, srcjar)

maven_jar = repository_rule(
    attrs = {
        "artifact": attr.string(mandatory = True),
        "sha1": attr.string(),
        "src_sha1": attr.string(),
        "_download_script": attr.label(default = Label("//tools:download_file.py")),
        "repository": attr.string(default = MAVEN_CENTRAL),
        "attach_source": attr.bool(default = True),
        "unsign": attr.bool(default = False),
        "deps": attr.string_list(),
        "exports": attr.string_list(),
        "exclude": attr.string_list(),
    },
    local = True,
    implementation = _maven_jar_impl,
)
