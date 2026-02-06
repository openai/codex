def _rbe_platform_repo_impl(rctx):
    arch = rctx.os.arch
    if arch in ["x86_64", "amd64"]:
        cpu = "x86_64"
        exec_arch = "amd64"
        image_sha = "8c9ff94187ea7c08a31e9a81f5fe8046ea3972a6768983c955c4079fa30567fb"
    elif arch in ["aarch64", "arm64"]:
        cpu = "aarch64"
        exec_arch = "arm64"
        image_sha = "ad9506086215fccfc66ed8d2be87847324be56790ae6a1964c241c28b77ef141"
    else:
        fail("Unsupported host arch for rbe platform: {}".format(arch))

    rctx.file("BUILD.bazel", """\
platform(
    name = "rbe_platform",
    constraint_values = [
        "@platforms//cpu:{cpu}",
        "@platforms//os:linux",
        "@bazel_tools//tools/cpp:clang",
        "@toolchains_llvm_bootstrapped//constraints/libc:gnu.2.28",
    ],
    exec_properties = {{
        # Ubuntu-based image that includes git, python3, dotslash, and other
        # tools that various integration tests need.
        # Verify at https://hub.docker.com/layers/mbolin491/codex-bazel/latest/images/sha256:{image_sha}
        "container-image": "docker://docker.io/mbolin491/codex-bazel@sha256:{image_sha}",
        "Arch": "{arch}",
        "OSFamily": "Linux",
    }},
    visibility = ["//visibility:public"],
)
""".format(
    cpu = cpu,
    arch = exec_arch,
    image_sha = image_sha
))

rbe_platform_repository = repository_rule(
    implementation = _rbe_platform_repo_impl,
    doc = "Sets up a platform for remote builds with an Arch exec_property matching the host.",
)
