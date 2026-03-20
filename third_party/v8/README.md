# `rusty_v8` Consumer Artifacts

This directory wires the `v8` crate to exact-version prebuilt artifacts.
Consumer builds use:

- upstream `denoland/rusty_v8` release archives on Darwin, GNU Linux, and Windows
- `openai/codex` release assets for `x86_64-unknown-linux-musl` and
  `aarch64-unknown-linux-musl`

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
It only builds musl release pairs from source:

- `//third_party/v8:rusty_v8_release_pair_x86_64_unknown_linux_musl`
- `//third_party/v8:rusty_v8_release_pair_aarch64_unknown_linux_musl`

Cargo musl builds use `RUSTY_V8_ARCHIVE` plus a downloaded
`RUSTY_V8_SRC_BINDING_PATH` to point at those `openai/codex` release assets
directly. We do not use `RUSTY_V8_MIRROR` for musl because the upstream `v8`
crate hardcodes a `v<crate_version>` tag layout, while our musl artifacts are
published under `rusty-v8-v<crate_version>`.

Do not mix artifacts across crate versions. The archive and binding must match
the exact resolved `v8` crate version in `codex-rs/Cargo.lock`.
