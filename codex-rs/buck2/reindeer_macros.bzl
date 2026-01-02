load("@prelude//rust:cargo_buildscript.bzl", _prelude_buildscript_run = "buildscript_run")
load("@prelude//rust:cargo_package.bzl", "cargo")


def codex_noop_alias(**_kwargs):
    # Reindeer normally emits aliases like `alias(name = "rand", actual = ":rand-0.8.5")`
    # to provide stable unversioned target names. In a non-trivial workspace it's
    # common to have multiple versions of the same crate in one graph, which
    # leads to duplicate alias target names and buckification failures.
    #
    # For local Buck experiments (where we don't check in generated third-party
    # BUCK files), it's simplest to disable these aliases and depend on
    # versioned targets directly.
    pass


def _codex_extra_srcs_for_manifest_dir(manifest_dir):
    if not manifest_dir:
        return []

    # Use per-crate globs rooted at the crate's vendored manifest dir, which is
    # passed through by Reindeer as CARGO_MANIFEST_DIR (e.g. vendor/foo-1.2.3).
    #
    # We include *all* files under the crate root so `include_str!` and
    # `include_bytes!` work without per-crate Reindeer fixups. This is local-only
    # buckification, so we prefer robustness over a minimal srcs list.
    return glob(
        ["{}/**".format(manifest_dir)],
        exclude = [
            "{}/target/**".format(manifest_dir),
            "{}/.git/**".format(manifest_dir),
        ],
    )


def codex_rust_library(**kwargs):
    # Make generated third-party targets consumable from anywhere in the repo.
    kwargs["visibility"] = ["PUBLIC"]
    env = kwargs.get("env", {})
    manifest_dir = env.get("CARGO_MANIFEST_DIR")
    srcs = list(kwargs.get("srcs", []))
    srcs.extend(_codex_extra_srcs_for_manifest_dir(manifest_dir))
    kwargs["srcs"] = srcs
    cargo.rust_library(**kwargs)


def codex_rust_binary(**kwargs):
    kwargs["visibility"] = ["PUBLIC"]
    env = kwargs.get("env", {})
    manifest_dir = env.get("CARGO_MANIFEST_DIR")
    srcs = list(kwargs.get("srcs", []))
    srcs.extend(_codex_extra_srcs_for_manifest_dir(manifest_dir))
    kwargs["srcs"] = srcs
    cargo.rust_binary(**kwargs)


def codex_buildscript_run(**kwargs):
    # Many build scripts (especially those using `cc`/`cc-rs`) expect Cargo to
    # provide a handful of profile env vars. Buck does not set these by default.
    env = dict(kwargs.get("env", {}))
    rust_profile = read_config("codex", "rust_profile", "dev")
    if rust_profile == "release":
        env.setdefault("OPT_LEVEL", "3")
        env.setdefault("PROFILE", "release")
        env.setdefault("DEBUG", "false")
    else:
        env.setdefault("OPT_LEVEL", "0")
        env.setdefault("PROFILE", "debug")
        env.setdefault("DEBUG", "true")

    # Provide common Cargo cfg env vars that some build scripts expect.
    env.setdefault(
        "CARGO_CFG_TARGET_OS",
        select({
            "prelude//os:linux": "linux",
            "prelude//os:macos": "macos",
            "prelude//os:windows": "windows",
            "DEFAULT": "",
        }),
    )
    env.setdefault(
        "CARGO_CFG_TARGET_ARCH",
        select({
            "prelude//cpu:arm64": "aarch64",
            "prelude//cpu:x86_64": "x86_64",
            "DEFAULT": "",
        }),
    )
    env.setdefault("CARGO_CFG_TARGET_ENDIAN", "little")
    env.setdefault(
        "CARGO_CFG_TARGET_ENV",
        select({
            "prelude//os:linux": "gnu",
            "DEFAULT": "",
        }),
    )

    # Forward native link directives emitted by build scripts into rustc flags.
    # Without this, crates like `ring` and `tree-sitter-*` will compile but fail
    # to link due to missing native symbols.
    kwargs.setdefault("rustc_link_lib", True)
    kwargs.setdefault("rustc_link_search", True)

    # `CARGO_MANIFEST_DIR` is expected by many build scripts. We can usually
    # derive it from the `manifest_dir` parameter that buildscript_run uses.
    manifest_dir = kwargs.get("manifest_dir")
    if type(manifest_dir) == type(""):
        env.setdefault("CARGO_MANIFEST_DIR", "$(location {})".format(manifest_dir))

    kwargs["env"] = env
    _prelude_buildscript_run(**kwargs)
