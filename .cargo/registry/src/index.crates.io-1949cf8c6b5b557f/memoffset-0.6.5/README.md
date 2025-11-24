# memoffset #

[![](https://img.shields.io/crates/v/memoffset.svg)](https://crates.io/crates/memoffset)

C-Like `offset_of` functionality for Rust structs.

Introduces the following macros:
 * `offset_of!` for obtaining the offset of a member of a struct.
 * `offset_of_tuple!` for obtaining the offset of a member of a tuple. (Requires Rust 1.20+)
 * `span_of!` for obtaining the range that a field, or fields, span.

`memoffset` works under `no_std` environments.

## Usage ##
Add the following dependency to your `Cargo.toml`:

```toml
[dependencies]
memoffset = "0.6"
```

These versions will compile fine with rustc versions greater or equal to 1.19.

## Examples ##
```rust
use memoffset::{offset_of, span_of};

#[repr(C, packed)]
struct Foo {
    a: u32,
    b: u32,
    c: [u8; 5],
    d: u32,
}

fn main() {
    assert_eq!(offset_of!(Foo, b), 4);
    assert_eq!(offset_of!(Foo, d), 4+4+5);

    assert_eq!(span_of!(Foo, a),        0..4);
    assert_eq!(span_of!(Foo, a ..  c),  0..8);
    assert_eq!(span_of!(Foo, a ..= c),  0..13);
    assert_eq!(span_of!(Foo, ..= d),    0..17);
    assert_eq!(span_of!(Foo, b ..),     4..17);
}
```

## Feature flags ##

### Usage in constants ###
`memoffset` has **experimental** support for compile-time `offset_of!` on a nightly compiler.

In order to use it, you must enable the `unstable_const` crate feature and several compiler features.

Cargo.toml:
```toml
[dependencies.memoffset]
version = "0.6"
features = ["unstable_const"]
```

Your crate root: (`lib.rs`/`main.rs`)
```rust,ignore
#![feature(const_ptr_offset_from, const_refs_to_cell)]
```
