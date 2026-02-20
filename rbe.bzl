def _rbe_platform_repo_impl(rctx):
    arch = rctx.os.arch
    if arch in ["x86_64", "amd64"]:
        cpu = "x86_64"
        exec_arch = "amd64"
        image_sha = "0a8e56bfaa3b2e5279db0d3bb2d62b44ba5e5d63a37d97eb8790f49da570af70"
    elif arch in ["aarch64", "arm64"]:
        cpu = "aarch64"
        exec_arch = "arm64"
        image_sha = "136487cc4b7cf6f1021816ca18ed00896daed98404ea91dc4d6dd9e9d1cf9564"
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
