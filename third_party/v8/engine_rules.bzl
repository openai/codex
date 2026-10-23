"""Codex's fixed selection for branches of the imported V8 source inventory."""

load("@v8//:bazel/defs.bzl", "FlagInfo")

def _options_impl(ctx):
    return FlagInfo(value = ctx.build_setting_value)

# Source-only features keep upstream select names without adding CLI options.
# ABI/runtime settings are still V8 release inputs until artifact profiles take over.
v8_fixed_source_features = rule(
    implementation = _options_impl,
    build_setting = config.bool(flag = False),
)
