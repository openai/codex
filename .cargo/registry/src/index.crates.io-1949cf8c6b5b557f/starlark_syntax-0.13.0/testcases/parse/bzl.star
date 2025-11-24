# @generated
load(":workspace.bzl", _mvn_deps="maven_dependencies")

def _declare_maven(item):
  sha = item.get("sha1")
  kwargs = {
    "name": item["name"],
    "artifact": item["artifact"],
    "repository": item["repository"],
  }
  if sha != None:
    kwargs["sha1"] = sha
  native.maven_jar(**kwargs)

def maven_dependencies():
  _mvn_deps(_declare_maven)
