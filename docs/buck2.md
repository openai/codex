# Buck2 (Experimental, Local-Only)

This repo has an **experimental** Buck2 + Reindeer setup for `codex-rs`.

For now, this is intended for **local development only**. We are not ready to:
- commit generated `BUCK` files for first-party crates
- commit the vendored third-party crate sources under `codex-rs/third-party/`

Those artifacts are intentionally gitignored to avoid repo bloat while we
evaluate performance and developer experience.

## Prereqs

- A working Rust toolchain (see `codex-rs/rust-toolchain.toml`).
- Buck2 + Reindeer are pinned via:
  - `./scripts/buck2`
  - `./scripts/reindeer`

## One-Time Setup (Generates Local-Only Files)

Run:

```sh
./scripts/setup_buck2_local.sh
```

This script will:
- vendor third-party crates into `codex-rs/third-party/vendor/` (large)
- run `reindeer` to produce third-party Buck targets (gitignored)
- generate first-party `BUCK` files for workspace members in `codex-rs/**/BUCK` (gitignored)
- format generated `BUCK` files with the pinned `./scripts/buildifier`

Notes:
- `codex-rs/third-party/` is currently large (on the order of GBs) due to vendoring.
- All generated artifacts live under gitignored paths (see `.gitignore`).

## Building

Buck targets are rooted at the repo root.

Dev build (default):

```sh
./scripts/buck2 build //codex-rs/cli:codex
```

Release-ish build:

```sh
./scripts/buck2 build -c codex.rust_profile=release //codex-rs/cli:codex
```

The `codex.rust_profile` config knob is defined in `.buckconfig` and is used to
approximate Cargo profiles.

## Testing

This setup generates `rust_test()` rules for first-party crates, so you can run:

```sh
./scripts/buck2 test //codex-rs/...
```

Targeted runs are often more useful:

```sh
./scripts/buck2 test //codex-rs/cli:codex-cli-unit-tests
```

### Current Limitations

This is still a work-in-progress, so some tests may fail under Buck2 even if
they pass under Cargo. Common reasons:
- Integration tests that assume `cargo test` semantics (e.g. using `escargot` to
  invoke Cargo during the test).
- Snapshot tests (via `insta`) can behave differently because Buck2 executes
  tests under an isolated sandbox with project-relative paths.

If you see failures, prefer running a smaller target (like a single crate’s unit
tests) while we iterate on broader compatibility.

## Cleaning Up

To delete Buck2 outputs/caches for this repo:

```sh
./scripts/buck2 clean
```

To reclaim disk space from the vendored crates, remove:

```sh
rm -rf codex-rs/third-party
```

## Repo Layout Notes

- The repo root is the Buck root (`.buckroot`).
- `prelude/` is a tiny placeholder directory so `.buckconfig` can declare the
  `prelude` cell while using Buck2’s bundled prelude implementation.
