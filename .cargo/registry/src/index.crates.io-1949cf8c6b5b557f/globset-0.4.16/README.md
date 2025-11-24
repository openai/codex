globset
=======
Cross platform single glob and glob set matching. Glob set matching is the
process of matching one or more glob patterns against a single candidate path
simultaneously, and returning all of the globs that matched.

[![Build status](https://github.com/BurntSushi/ripgrep/workflows/ci/badge.svg)](https://github.com/BurntSushi/ripgrep/actions)
[![](https://img.shields.io/crates/v/globset.svg)](https://crates.io/crates/globset)

Dual-licensed under MIT or the [UNLICENSE](https://unlicense.org/).

### Documentation

[https://docs.rs/globset](https://docs.rs/globset)

### Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
globset = "0.4"
```

### Features

* `serde1`: Enables implementing Serde traits on the `Glob` type.

### Example: one glob

This example shows how to match a single glob against a single file path.

```rust
use globset::Glob;

let glob = Glob::new("*.rs")?.compile_matcher();

assert!(glob.is_match("foo.rs"));
assert!(glob.is_match("foo/bar.rs"));
assert!(!glob.is_match("Cargo.toml"));
```

### Example: configuring a glob matcher

This example shows how to use a `GlobBuilder` to configure aspects of match
semantics. In this example, we prevent wildcards from matching path separators.

```rust
use globset::GlobBuilder;

let glob = GlobBuilder::new("*.rs")
    .literal_separator(true).build()?.compile_matcher();

assert!(glob.is_match("foo.rs"));
assert!(!glob.is_match("foo/bar.rs")); // no longer matches
assert!(!glob.is_match("Cargo.toml"));
```

### Example: match multiple globs at once

This example shows how to match multiple glob patterns at once.

```rust
use globset::{Glob, GlobSetBuilder};

let mut builder = GlobSetBuilder::new();
// A GlobBuilder can be used to configure each glob's match semantics
// independently.
builder.add(Glob::new("*.rs")?);
builder.add(Glob::new("src/lib.rs")?);
builder.add(Glob::new("src/**/foo.rs")?);
let set = builder.build()?;

assert_eq!(set.matches("src/bar/baz/foo.rs"), vec![0, 2]);
```

### Performance

This crate implements globs by converting them to regular expressions, and
executing them with the
[`regex`](https://github.com/rust-lang/regex)
crate.

For single glob matching, performance of this crate should be roughly on par
with the performance of the
[`glob`](https://github.com/rust-lang/glob)
crate. (`*_regex` correspond to benchmarks for this library while `*_glob`
correspond to benchmarks for the `glob` library.)
Optimizations in the `regex` crate may propel this library past `glob`,
particularly when matching longer paths.

```
test ext_glob             ... bench:         425 ns/iter (+/- 21)
test ext_regex            ... bench:         175 ns/iter (+/- 10)
test long_glob            ... bench:         182 ns/iter (+/- 11)
test long_regex           ... bench:         173 ns/iter (+/- 10)
test short_glob           ... bench:          69 ns/iter (+/- 4)
test short_regex          ... bench:          83 ns/iter (+/- 2)
```

The primary performance advantage of this crate is when matching multiple
globs against a single path. With the `glob` crate, one must match each glob
synchronously, one after the other. In this crate, many can be matched
simultaneously. For example:

```
test many_short_glob      ... bench:       1,063 ns/iter (+/- 47)
test many_short_regex_set ... bench:         186 ns/iter (+/- 11)
```

### Comparison with the [`glob`](https://github.com/rust-lang/glob) crate

* Supports alternate "or" globs, e.g., `*.{foo,bar}`.
* Can match non-UTF-8 file paths correctly.
* Supports matching multiple globs at once.
* Doesn't provide a recursive directory iterator of matching file paths,
  although I believe this crate should grow one eventually.
* Supports case insensitive and require-literal-separator match options, but
  **doesn't** support the require-literal-leading-dot option.
