byteorder
=========
This crate provides convenience methods for encoding and decoding
numbers in either big-endian or little-endian order.

[![Build status](https://github.com/BurntSushi/byteorder/workflows/ci/badge.svg)](https://github.com/BurntSushi/byteorder/actions)
[![crates.io](https://img.shields.io/crates/v/byteorder.svg)](https://crates.io/crates/byteorder)

Dual-licensed under MIT or the [UNLICENSE](https://unlicense.org/).


### Documentation

https://docs.rs/byteorder


### Installation

This crate works with Cargo and is on
[crates.io](https://crates.io/crates/byteorder). Add it to your `Cargo.toml`
like so:

```toml
[dependencies]
byteorder = "1"
```

If you want to augment existing `Read` and `Write` traits, then import the
extension methods like so:

```rust
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian, LittleEndian};
```

For example:

```rust
use std::io::Cursor;
use byteorder::{BigEndian, ReadBytesExt};

let mut rdr = Cursor::new(vec![2, 5, 3, 0]);
// Note that we use type parameters to indicate which kind of byte order
// we want!
assert_eq!(517, rdr.read_u16::<BigEndian>().unwrap());
assert_eq!(768, rdr.read_u16::<BigEndian>().unwrap());
```

### `no_std` crates

This crate has a feature, `std`, that is enabled by default. To use this crate
in a `no_std` context, add the following to your `Cargo.toml`:

```toml
[dependencies]
byteorder = { version = "1", default-features = false }
```


### Minimum Rust version policy

This crate's minimum supported `rustc` version is `1.60.0`.

The current policy is that the minimum Rust version required to use this crate
can be increased in minor version updates. For example, if `crate 1.0` requires
Rust 1.20.0, then `crate 1.0.z` for all values of `z` will also require Rust
1.20.0 or newer. However, `crate 1.y` for `y > 0` may require a newer minimum
version of Rust.

In general, this crate will be conservative with respect to the minimum
supported version of Rust.


### Alternatives

Note that as of Rust 1.32, the standard numeric types provide built-in methods
like `to_le_bytes` and `from_le_bytes`, which support some of the same use
cases.
