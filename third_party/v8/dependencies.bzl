"""Pinned source imports for Codex's V8 dependency builds.

V8 uses these same repositories through the codex_deps module-extension bridge
in its MODULE.bazel. The BUILD files, and all dependency compilation policy,
live next to this file so updating V8 does not require patching shared rules.

On V8 upgrades follow README.md#updating-source-dependencies. Highway, ICU,
dragonbox, FP16 and fast_float follow the pinned V8 DEPS; simdutf matches the
version in Chromium's vendored metadata. Abseil follows V8's unpatched MODULE.
Source versions are reviewed manually, not by the prebuilt checksum CI check.
"""

load("@bazel_tools//tools/build_defs/repo:git.bzl", "git_repository")
load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load(":abseil_repository.bzl", "v8_abseil_repository")

def _dependencies_impl(module_ctx):
    v8_abseil_repository(name = "v8_abseil")

    http_archive(
        name = "v8_highway",
        build_file = Label("//third_party/v8:highway.BUILD.bazel"),
        sha256 = "741d705781e0b3e406beda8f1f994fbae01321237ce8023a1ad90fbaf7940c25",
        strip_prefix = "highway-2607d3b5b0113992fe84d3848859eae13b3b52c1",
        urls = ["https://github.com/google/highway/archive/2607d3b5b0113992fe84d3848859eae13b3b52c1.tar.gz"],
    )

    git_repository(
        name = "v8_icu",
        build_file = Label("//third_party/v8:icu.BUILD.bazel"),
        commit = "d578f2e8b7bd5938e21cfb6bf15c079e0aa5b738",
        patch_cmds = ["find source -name BUILD.bazel | xargs rm"],
        patch_cmds_win = ["Get-ChildItem -Path source -File -Include BUILD.bazel -Recurse | Remove-Item"],
        remote = "https://chromium.googlesource.com/chromium/deps/icu.git",
    )

    http_archive(
        name = "v8_fast_float",
        build_file = Label("//third_party/v8:fast_float.BUILD.bazel"),
        sha256 = "76f958dd97b1cf4d8862d1f0986a47d4bdfa8845252bae15ef0f40de3b95961f",
        strip_prefix = "fast_float-8.2.10",
        urls = ["https://github.com/fastfloat/fast_float/archive/refs/tags/v8.2.10.tar.gz"],
    )

    http_archive(
        name = "v8_simdutf",
        build_file = Label("//third_party/v8:simdutf.BUILD.bazel"),
        sha256 = "0180de81a1dd48a87b8c0442ffa81734f3db91a7350914107a449935124e3c6f",
        strip_prefix = "simdutf-7.7.0",
        urls = ["https://github.com/simdutf/simdutf/archive/refs/tags/v7.7.0.tar.gz"],
    )

    git_repository(
        name = "v8_dragonbox",
        build_file = Label("//third_party/v8:dragonbox.BUILD.bazel"),
        commit = "beeeef91cf6fef89a4d4ba5e95d47ca64ccb3a44",
        remote = "https://chromium.googlesource.com/external/github.com/jk-jeon/dragonbox.git",
    )

    git_repository(
        name = "v8_fp16",
        build_file = Label("//third_party/v8:fp16.BUILD.bazel"),
        commit = "782eea126dc5c755827be751a099eb01826175cf",
        remote = "https://chromium.googlesource.com/external/github.com/Maratyszcza/FP16.git",
    )

    return module_ctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = "all",
        root_module_direct_dev_deps = [],
    )

dependencies = module_extension(implementation = _dependencies_impl)
