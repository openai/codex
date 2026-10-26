"""Runtime settings for Codex's Highway and simdutf V8 dependencies.

These targets build with the same Chromium libc++ headers as V8 when that
profile is active. Shared libraries such as Abseil remain separate.
"""

RUSTY_V8_DEPENDENCY_COPTS = select({
    "@@//third_party/v8:use_rusty_v8_custom_libcxx_native_windows": [
        "/D_LIBCPP_DISABLE_VISIBILITY_ANNOTATIONS",
        "/D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_EXTENSIVE",
        "/D_LIBCPP_INSTRUMENTED_WITH_ASAN=0",
    ],
    "@@//third_party/v8:use_rusty_v8_custom_libcxx": [
        "-nostdinc++",
        "-D_LIBCPP_DISABLE_VISIBILITY_ANNOTATIONS",
        "-D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_EXTENSIVE",
        "-D_LIBCPP_INSTRUMENTED_WITH_ASAN=0",
        "-D_LIBCXXABI_DISABLE_VISIBILITY_ANNOTATIONS",
    ],
    "//conditions:default": [],
})

RUSTY_V8_DEPENDENCY_DEPS = select({
    "@@//third_party/v8:use_rusty_v8_custom_libcxx": [
        "@@//third_party/v8:rusty_v8_custom_libcxx_headers",
    ],
    "//conditions:default": [],
})
