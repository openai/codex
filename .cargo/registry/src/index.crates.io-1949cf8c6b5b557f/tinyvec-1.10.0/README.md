[![License:Zlib](https://img.shields.io/badge/License-Zlib-brightgreen.svg)](https://opensource.org/licenses/Zlib)
![Minimum Rust Version](https://img.shields.io/badge/Min%20Rust-1.47-green.svg)
[![crates.io](https://img.shields.io/crates/v/tinyvec.svg)](https://crates.io/crates/tinyvec)
[![docs.rs](https://docs.rs/tinyvec/badge.svg)](https://docs.rs/tinyvec/)

![Unsafe-Zero-Percent](https://img.shields.io/badge/Unsafety-0%25-brightgreen.svg)

# tinyvec

A 100% safe crate of vec-like types.
Not just safe at the public API boundary, fully safe for all internal code too: `#![forbid(unsafe_code)]`

The provided types are as follows:
* `ArrayVec` is an array-backed vec-like data structure. It panics on overflow.
* `SliceVec` is similar, but using a `&mut [T]` as the data backing.
* `TinyVec` (`alloc` feature) is an enum that's either an `Inline(ArrayVec)` or a `Heap(Vec)`.
  If a `TinyVec` is `Inline` and would overflow its array it automatically transitions to `Heap` and continues whatever it was doing.

To attain this "100% safe code" status there is one compromise: the element type of the vecs must implement `Default`.

For more API details, please see [the docs.rs documentation](https://docs.rs/tinyvec/)

## `tinyvec` Alternatives?

Maybe you don't want to use `tinyvec`, there's other crates you might use instead!

* [arrayvec](https://docs.rs/arrayvec) is a crate with array-backed structures.
* [smallvec](https://docs.rs/smallvec) is a crate where the array-backed data can be moved to the heap on overflow.

The main difference is that both of those crates use `unsafe` code.
This mostly allows them to get rid of the `Default` limitation for elements that `tinyvec` imposes.
The `smallvec` and `arrayvec` crates are generally correct, but there's been occasional bugs leading to UB.
With `tinyvec`, any uncaught bugs *can't* lead to UB, because the crate is safe code all the way through.
If you want that absolute level of assurance against UB, use `tinyvec`.
