[![crates.io](https://img.shields.io/crates/v/multimap.svg)](https://crates.io/crates/multimap)
[![docs.rs](https://docs.rs/multimap/badge.svg)](https://docs.rs/multimap)

# Multimap implementation for Rust

This is a multimap implementation for Rust. Implemented as a thin wrapper around
`std::collections::HashMap`.

## Example

````rust
extern crate multimap;

use multimap::MultiMap;

fn main () {
    let mut map = MultiMap::new();

    map.insert("key1", 42);
    map.insert("key1", 1337);
    map.insert("key2", 2332);

    assert_eq!(map["key1"], 42);
    assert_eq!(map.get("key1"), Some(&42));
    assert_eq!(map.get_vec("key1"), Some(&vec![42, 1337]));
}
````

## Changelog

### 0.10.1

* Fix docs for flat_iter #45
* Clippy warnings

### 0.10.0

* Added `FromIterator<(K, Vec<V>)>` [#48](https://github.com/havarnov/multimap/pull/48).

### 0.9.1

* Fixes a bug where iteration would panic on empty (inner) vectors [#46](https://github.com/havarnov/multimap/issues/46).

### 0.9.0

* Added ```flat_iter``` and ```flat_iter_mut```
* Fixed bug where ```get``` and ```get_mut``` could panic.

### 0.8.3

* `multimap!` macro fixes; allow trailing comma, naming hygiene and create with
  enough capacity for all elements.

### 0.8.2

* Added ```#![forbid(unsafe_code)]```.

### 0.8.1

* Fixed wrong link to documentation in `Cargo.toml`.

### 0.8.0

* Added ```MultiMap::insert_many```.
* Added ```MultiMap::insert_many_from_slice```.

### 0.7.0

* Added possibility to replace the default hasher for the underlying
  ```HashMap```.
* Fix build warning by removing an unnecessary ```mut```.

## License

Licensed under either of
 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)
at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
