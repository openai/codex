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

Run the lint against `codex-rs` from the repo root:

```bash
./tools/argument-comment-lint/run.sh -p codex-core
just argument-comment-lint -p codex-core
```

If no package selection is provided, `run.sh` defaults to checking the
`codex-rs` workspace with `--workspace --no-deps`.

Repo runs also promote `uncommented_anonymous_literal_argument` to an error by
default:

```bash
./tools/argument-comment-lint/run.sh -p codex-core
```

The wrapper does that by setting `DYLINT_RUSTFLAGS`, and it leaves an explicit
existing setting alone. It also defaults `CARGO_INCREMENTAL=0` unless you have
already set it, because the current nightly Dylint flow can otherwise hit a
rustc incremental compilation ICE locally. To override that behavior for an ad
hoc run:

```bash
DYLINT_RUSTFLAGS="-A uncommented-anonymous-literal-argument" \
CARGO_INCREMENTAL=1 \
  ./tools/argument-comment-lint/run.sh -p codex-core
```

To expand target coverage for an ad hoc run:

```bash
./tools/argument-comment-lint/run.sh -p codex-core -- --all-targets
```
