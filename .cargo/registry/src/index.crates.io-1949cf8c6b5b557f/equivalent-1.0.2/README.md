# Equivalent

[![crates.io](https://img.shields.io/crates/v/equivalent.svg)](https://crates.io/crates/equivalent)
[![docs](https://docs.rs/equivalent/badge.svg)](https://docs.rs/equivalent)

`Equivalent` and `Comparable` are Rust traits for key comparison in maps.

These may be used in the implementation of maps where the lookup type `Q`
may be different than the stored key type `K`.

* `Q: Equivalent<K>` checks for equality, similar to the `HashMap<K, V>`
  constraint `K: Borrow<Q>, Q: Eq`.
* `Q: Comparable<K>` checks the ordering, similar to the `BTreeMap<K, V>`
  constraint `K: Borrow<Q>, Q: Ord`.

These traits are not used by the maps in the standard library, but they may
add more flexibility in third-party map implementations, especially in
situations where a strict `K: Borrow<Q>` relationship is not available.

## License

Equivalent is distributed under the terms of both the MIT license and the
Apache License (Version 2.0). See [LICENSE-APACHE](LICENSE-APACHE) and
[LICENSE-MIT](LICENSE-MIT) for details. Opening a pull request is
assumed to signal agreement with these licensing terms.
