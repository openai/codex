# `rusty_v8` Artifact Placeholder

The Bazel label `//third_party/v8:rusty_v8_archive` currently resolves to this placeholder file.

Replace the filegroup target in `third_party/v8/BUILD.bazel` with the real musl-built
`librusty_v8` archive before enabling the `codex-v8-poc` crate's `rusty_v8` feature for musl
targets.

Expected artifact examples:

- `librusty_v8_release_x86_64-unknown-linux-musl.a`
- `librusty_v8_release_aarch64-unknown-linux-musl.a`

Non-musl Bazel builds intentionally do not receive `RUSTY_V8_ARCHIVE`; they fall back to
`rusty_v8`'s default build-script behavior instead.
