"""Apply V8's private runtime policy after a generator's exec transition."""

load("@rules_cc//cc:cc_binary.bzl", "cc_binary")
load("@with_cfg.bzl", "with_cfg")

_PRIVATE_RUNTIME = select({
    str(Label("//third_party/v8:use_rusty_v8_custom_libcxx")): True,
    "//conditions:default": False,
})

# LLVM's flags are target-scoped. V8 generators intentionally share V8's
# private libc++ profile, but unrelated tools must keep their default runtime.
v8_cc_binary, _v8_cc_binary_internal = with_cfg(cc_binary).set(
    Label("@llvm//config:nostdincxx"),
    _PRIVATE_RUNTIME,
).set(
    Label("@llvm//config:nostdlibxx"),
    _PRIVATE_RUNTIME,
).build()
