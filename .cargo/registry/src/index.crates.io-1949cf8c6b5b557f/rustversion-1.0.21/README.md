Compiler version cfg
====================

[<img alt="github" src="https://img.shields.io/badge/github-dtolnay/rustversion-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/dtolnay/rustversion)
[<img alt="crates.io" src="https://img.shields.io/crates/v/rustversion.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/rustversion)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-rustversion-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/rustversion)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/dtolnay/rustversion/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/dtolnay/rustversion/actions?query=branch%3Amaster)

This crate provides macros for conditional compilation according to rustc
compiler version, analogous to [`#[cfg(...)]`][cfg] and
[`#[cfg_attr(...)]`][cfg_attr].

[cfg]: https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg-attribute
[cfg_attr]: https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg_attr-attribute

```toml
[dependencies]
rustversion = "1.0"
```

<br>

## Selectors

- <b>`#[rustversion::stable]`</b>
  —<br>
  True on any stable compiler.

- <b>`#[rustversion::stable(1.34)]`</b>
  —<br>
  True on exactly the specified stable compiler.

- <b>`#[rustversion::beta]`</b>
  —<br>
  True on any beta compiler.

- <b>`#[rustversion::nightly]`</b>
  —<br>
  True on any nightly compiler or dev build.

- <b>`#[rustversion::nightly(2025-01-01)]`</b>
  —<br>
  True on exactly one nightly.

- <b>`#[rustversion::since(1.34)]`</b>
  —<br>
  True on that stable release and any later compiler, including beta and
  nightly.

- <b>`#[rustversion::since(2025-01-01)]`</b>
  —<br>
  True on that nightly and all newer ones.

- <b>`#[rustversion::before(`</b><i>version or date</i><b>`)]`</b>
  —<br>
  Negative of *#[rustversion::since(...)]*.

- <b>`#[rustversion::not(`</b><i>selector</i><b>`)]`</b>
  —<br>
  Negative of any selector; for example *#[rustversion::not(nightly)]*.

- <b>`#[rustversion::any(`</b><i>selectors...</i><b>`)]`</b>
  —<br>
  True if any of the comma-separated selectors is true; for example
  *#[rustversion::any(stable, beta)]*.

- <b>`#[rustversion::all(`</b><i>selectors...</i><b>`)]`</b>
  —<br>
  True if all of the comma-separated selectors are true; for example
  *#[rustversion::all(since(1.31), before(1.34))]*.

- <b>`#[rustversion::attr(`</b><i>selector</i><b>`, `</b><i>attribute</i><b>`)]`</b>
  —<br>
  For conditional inclusion of attributes; analogous to `cfg_attr`.

- <b>`rustversion::cfg!(`</b><i>selector</i><b>`)`</b>
  —<br>
  An expression form of any of the above attributes; for example
  *if rustversion::cfg!(any(stable, beta)) { ... }*.

<br>

## Use cases

Providing additional trait impls as types are stabilized in the standard library
without breaking compatibility with older compilers; in this case Pin\<P\>
stabilized in [Rust 1.33][pin]:

[pin]: https://blog.rust-lang.org/2019/02/28/Rust-1.33.0.html#pinning

```rust
#[rustversion::since(1.33)]
use std::pin::Pin;

#[rustversion::since(1.33)]
impl<P: MyTrait> MyTrait for Pin<P> {
    /* ... */
}
```

Similar but for language features; the ability to control alignment greater than
1 of packed structs was stabilized in [Rust 1.33][packed].

[packed]: https://github.com/rust-lang/rust/blob/master/RELEASES.md#version-1330-2019-02-28

```rust
#[rustversion::attr(before(1.33), repr(packed))]
#[rustversion::attr(since(1.33), repr(packed(2)))]
struct Six(i16, i32);

fn main() {
    println!("{}", std::mem::align_of::<Six>());
}
```

Augmenting code with `const` as const impls are stabilized in the standard
library. This use of `const` as an attribute is recognized as a special case by
the rustversion::attr macro.

```rust
use std::time::Duration;

#[rustversion::attr(since(1.32), const)]
fn duration_as_days(dur: Duration) -> u64 {
    dur.as_secs() / 60 / 60 / 24
}
```

Emitting Cargo cfg directives from a build script. Note that this requires
listing `rustversion` under `[build-dependencies]` in Cargo.toml, not
`[dependencies]`.

```rust
// build.rs

fn main() {
    if rustversion::cfg!(since(1.36)) {
        println!("cargo:rustc-cfg=no_std");
    }
}
```

```rust
// src/lib.rs

#![cfg_attr(no_std, no_std)]

#[cfg(no_std)]
extern crate alloc;
```

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
