# `rusty_v8` Consumer Artifacts

This directory wires the `v8` crate to exact-version Bazel inputs.
Bazel consumer builds use:

- upstream `denoland/rusty_v8` release archives on Windows
- source-built V8 archives on Darwin, GNU Linux, and musl Linux
- `openai/codex` release assets for published musl release pairs

Cargo builds still use prebuilt `rusty_v8` archives by default. Only Bazel
overrides `RUSTY_V8_ARCHIVE`/`RUSTY_V8_SRC_BINDING_PATH` in `MODULE.bazel` to
select source-built local archives for its consumer builds.

Current pinned versions:

- Rust crate: `v8 = =146.4.0`
- Embedded upstream V8 source for musl release builds: `14.6.202.9`

The consumer-facing selectors are:

- `//third_party/v8:rusty_v8_archive_for_target`
- `//third_party/v8:rusty_v8_binding_for_target`

Musl release assets are expected at the tag:

- `rusty-v8-v<crate_version>`

with these raw asset names:

- `librusty_v8_release_<target>.a.gz`
- `src_binding_release_<target>.rs`

The dedicated publishing workflow is `.github/workflows/rusty-v8-release.yml`.
It builds musl release pairs from source and keeps the release artifacts as the
statically linked form:

- `//third_party/v8:rusty_v8_release_pair_x86_64_unknown_linux_musl`
- `//third_party/v8:rusty_v8_release_pair_aarch64_unknown_linux_musl`

Cargo musl builds use `RUSTY_V8_ARCHIVE` plus a downloaded
`RUSTY_V8_SRC_BINDING_PATH` to point at those `openai/codex` release assets
directly. We do not use `RUSTY_V8_MIRROR` for musl because the upstream `v8`
crate hardcodes a `v<crate_version>` tag layout, while our musl artifacts are
published under `rusty-v8-v<crate_version>`.

Do not mix artifacts across crate versions. The archive and binding must match
the exact resolved `v8` crate version in `codex-rs/Cargo.lock`.
