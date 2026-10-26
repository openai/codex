"""Owned V8 source selection and test metadata derived from the engine policy."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@v8//:bazel/defs.bzl", "FlagInfo", "build_config_content")

def _options_impl(ctx):
    return FlagInfo(value = ctx.build_setting_value)

# Source-only features keep upstream select names without adding CLI options.
# ABI/runtime selections are internal settings controlled by artifact profiles.
v8_fixed_source_features = rule(
    implementation = _options_impl,
    build_setting = config.bool(flag = False),
)

def _v8_build_config_impl(ctx):
    config = json.decode(build_config_content(json.encode(ctx.attr.arch), "true" if ctx.attr.icu else "false"))
    defines = ctx.attr.config[CcInfo].compilation_context.defines.to_list()
    config.update({
        "cppgc_microtask_queue": "V8_CPPGC_MICROTASK_QUEUE" in defines,
        "wasm_random_fuzzers": "V8_WASM_RANDOM_FUZZERS" in defines,
    })
    ctx.actions.write(ctx.outputs.out, json.encode(config) + "\n")
    return [DefaultInfo(files = depset([ctx.outputs.out]))]

_v8_build_config = rule(
    implementation = _v8_build_config_impl,
    attrs = {
        "arch": attr.string(mandatory = True),
        "config": attr.label(mandatory = True, providers = [CcInfo]),
        "icu": attr.bool(mandatory = True),
        "out": attr.output(mandatory = True),
    },
)

def v8_build_config(name, arch):
    """Keep upstream test metadata aligned with the owned platform features."""
    for mode in ["icu", "noicu"]:
        _v8_build_config(
            name = mode + "/" + name,
            arch = arch,
            config = ":define_flags",
            icu = mode == "icu",
            out = mode + "/" + name + ".json",
        )
