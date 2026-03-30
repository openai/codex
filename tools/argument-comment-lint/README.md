# argument-comment-lint

Isolated [Dylint](https://github.com/trailofbits/dylint) library for enforcing
Rust argument comments in the exact `/*param*/` shape.

Prefer self-documenting APIs over comment-heavy call sites when possible. If a
call site would otherwise read like `foo(false)` or `bar(None)`, consider an
enum, named helper, newtype, or another idiomatic Rust API shape first, and
use an argument comment only when a smaller compatibility-preserving change is
more appropriate.

It provides two lints:

- `argument_comment_mismatch` (`warn` by default): validates that a present
  `/*param*/` comment matches the resolved callee parameter name.
- `uncommented_anonymous_literal_argument` (`allow` by default): flags
  anonymous literal-like arguments such as `None`, `true`, `false`, and numeric
  literals when they do not have a preceding `/*param*/` comment.

String and char literals are exempt because they are often already
self-descriptive at the callsite.

## Behavior

Given:

```rust
fn create_openai_url(base_url: Option<String>, retry_count: usize) -> String {
    let _ = (base_url, retry_count);
    String::new()
}
```

This is accepted:

```rust
create_openai_url(/*base_url*/ None, /*retry_count*/ 3);
```

This is warned on by `argument_comment_mismatch`:

```rust
create_openai_url(/*api_base*/ None, 3);
```

This is only warned on when `uncommented_anonymous_literal_argument` is enabled:

```rust
create_openai_url(None, 3);
```

## Development

Install the required tooling once:

```bash
cargo install cargo-dylint dylint-link
rustup toolchain install nightly-2025-09-18 \
  --component llvm-tools-preview \
  --component rustc-dev \
  --component rust-src
```

Run the lint crate tests:

```bash
cd tools/argument-comment-lint
cargo test
```

GitHub releases also publish a DotSlash file named
`argument-comment-lint` for macOS arm64, Linux arm64, Linux x64, and Windows
x64. The published package contains a small runner executable, a bundled
`cargo-dylint`, and the prebuilt lint library.

The package is not a full Rust toolchain. Running the prebuilt path still
requires the pinned nightly toolchain to be installed via `rustup`:

```bash
rustup toolchain install nightly-2025-09-18 \
  --component llvm-tools-preview \
  --component rustc-dev \
  --component rust-src
```

The checked-in DotSlash file lives at `tools/argument-comment-lint/argument-comment-lint`.
`run-prebuilt-linter.py` resolves that file via `dotslash` and is the path used by
targeted package runs such as `just argument-comment-lint -p codex-core`.
Repo-wide runs now go through a native Bazel aspect that invokes a custom
`rustc_driver` and reuses Bazel-managed Rust dependency metadata instead of
spawning `cargo dylint` once per crate. The source-build path remains available
in `run.py` for people iterating on the lint crate itself.

The Unix archive layout is:

```text
argument-comment-lint/
  bin/
    argument-comment-lint
    cargo-dylint
  lib/
    libargument_comment_lint@nightly-2025-09-18-<target>.dylib|so
```

On Windows the same layout is published as a `.zip`, with `.exe` and `.dll`
filenames instead.

DotSlash resolves the package entrypoint to `argument-comment-lint/bin/argument-comment-lint`
(or `.exe` on Windows). That runner finds the sibling bundled `cargo-dylint`
binary and the single packaged Dylint library under `lib/`, normalizes the
host-qualified nightly filename to the plain `nightly-2025-09-18` channel when
needed, and then invokes `cargo-dylint dylint --lib-path <that-library>` with
the repo's default `DYLINT_RUSTFLAGS` and `CARGO_INCREMENTAL=0` settings.

The checked-in `run-prebuilt-linter.py` wrapper uses the fetched package
contents directly so the current checked-in alpha artifact works the same way.
It also makes sure the `rustup` shims stay ahead of any direct toolchain
`cargo` binary on `PATH`, and sets `RUSTUP_HOME` from `rustup show home` when
the environment does not already provide it. That extra `RUSTUP_HOME` export is
required for the current Windows Dylint driver path.

If you are changing the lint crate itself, use the source-build wrapper:

```bash
./tools/argument-comment-lint/run.py -p codex-core
```

Run the lint against `codex-rs` from the repo root:

```bash
just argument-comment-lint
bazel build --config=argument-comment-lint -- //codex-rs/...
./tools/argument-comment-lint/run-prebuilt-linter.py -p codex-core
just argument-comment-lint -p codex-core
```

If no package selection is provided, `just argument-comment-lint` now defaults
to the Bazel aspect path over `//codex-rs/...`. The Python wrappers remain the
package-scoped escape hatch and still default the underlying Cargo invocation
to `--all-targets` unless you explicitly narrow the target set, so targeted
wrapper runs cover test-only call sites by default.

Repo runs also promote `uncommented_anonymous_literal_argument` to an error by
default:

```bash
./tools/argument-comment-lint/run-prebuilt-linter.py -p codex-core
```

The wrapper does that by setting `DYLINT_RUSTFLAGS`, and it leaves an explicit
existing setting alone. It also defaults `CARGO_INCREMENTAL=0` unless you have
already set it, because the current nightly Dylint flow can otherwise hit a
rustc incremental compilation ICE locally. To override that behavior for an ad
hoc run:

```bash
DYLINT_RUSTFLAGS="-A uncommented-anonymous-literal-argument" \
CARGO_INCREMENTAL=1 \
  ./tools/argument-comment-lint/run.py -p codex-core
```

To override an explicitly narrow target selection, or to be explicit in scripts:

```bash
./tools/argument-comment-lint/run-prebuilt-linter.py -p codex-core -- --all-targets
```
