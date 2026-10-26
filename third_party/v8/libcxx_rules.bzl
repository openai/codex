"""Select the installed SDK-capable toolchain for native Windows libc++."""

load("@llvm//toolchain/runtimes:cc_runtime_library.bzl", "cc_runtime_stage0_library")
load("@rules_cc//cc:defs.bzl", "cc_library")

def libcxx_library(name, **kwargs):
    cc_runtime_stage0_library(
        name = name + "_stage0",
        **kwargs
    )
    cc_library(
        name = name + "_native_windows",
        linkstatic = True,
        target_compatible_with = select({
            "@//third_party/v8:use_rusty_v8_custom_libcxx_native_windows": [],
            "//conditions:default": ["@platforms//:incompatible"],
        }),
        **kwargs
    )
    native.alias(
        name = name,
        actual = select({
            "@//third_party/v8:use_rusty_v8_custom_libcxx_native_windows": ":" + name + "_native_windows",
            "//conditions:default": ":" + name + "_stage0",
        }),
    )
