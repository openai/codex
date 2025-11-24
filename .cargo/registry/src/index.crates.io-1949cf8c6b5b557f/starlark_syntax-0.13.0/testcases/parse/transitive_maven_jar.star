# @generated
MAX_TIMEOUT = 0x7FFFFFFF

def _validate_coordinate_length(coordinate):
    parts = coordinate.split(":")
    return len(parts) >= 2 and len(parts) <= 5

def _validate_coordinates(rctx):
    coordinates = rctx.attr.artifacts
    for coord in coordinates:
        if _validate_coordinate_length(coord) == False:
            fail("Invalid coordinate %s. Generally formatted as \"group:artifact:version\"" % coord)
    return True

def _create_arguments(rctx):
    arguments = ['--artifact ' + artifact for artifact in rctx.attr.artifacts]
    return ' '.join(arguments)

def _execute(rctx, command_string, quiet):
    return rctx.execute(["bash", "-c", command_string], timeout = rctx.attr._timeout, quiet = quiet)

def _transitive_maven_jar_impl(rctx):
    _validate_coordinates(rctx)
    arguments = _create_arguments(rctx)
    quiet = rctx.attr.quiet

    jar_path = rctx.path(rctx.attr._generate_workspace_tool)

    # execute the command
    result = _execute(rctx, "java -jar %s %s" % (jar_path, arguments), quiet)
    rctx.file('%s/BUILD' % rctx.path(''), '', False)

transitive_maven_jar = repository_rule(
        implementation = _transitive_maven_jar_impl,
        attrs = {
            "artifacts" : attr.string_list(default = [], mandatory = True),
            "quiet" : attr.bool(default = False, mandatory = False),
            "_timeout" : attr.int(default = MAX_TIMEOUT),
            "_generate_workspace_tool" : attr.label(executable = True, allow_files = True, cfg = "host", default = Label("//transitive_maven_jar:generate_workspace_deploy.jar"))
            #TODO(petros): add support for private repositories.
        },
        local = False,
)
