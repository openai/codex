# endi

[![Build Status](https://github.com/zeenix/endi/actions/workflows/rust.yml/badge.svg)](https://github.com/zeenix/endi/actions/workflows/rust.yml) [![API Documentation](https://docs.rs/endi/badge.svg)](https://docs.rs/endi/) [![crates.io](https://img.shields.io/crates/v/endi)](https://crates.io/crates/endi)

Yet another endian handling library for Rust. The approach is very similar to that of
`byteordered` crate with its `Endianness` enum, except that `endi` is much simpler and doesn't
depend on `byteorder` (or anything at all).

## Usage

The main type is `Endian` enum which can be either `Big` or `Little`. It provides various 
methods to read and write integers of different sizes and endianness.

```rust
use endi::{Endian, ReadBytes, WriteBytes};

let mut buf = [0u8; 4];
for endian in [Endian::Little, Endian::Big] {
    endian.write_u32(&mut buf, 0xAB_BA_FE_EF);
    assert_eq!(endian.read_u32(&buf), 0xAB_BA_FE_EF);

    // Using the `ReadBytes` and `WriteBytes` traits:
    let mut cursor = std::io::Cursor::new(&mut buf[..]);
    cursor.write_u32(endian, 0xAB_BA_FE_EF).unwrap();
    cursor.set_position(0);
    assert_eq!(cursor.read_u32(endian).unwrap(), 0xAB_BA_FE_EF);
}
```

## nostd

You can disable `std` by disabling the default `std` feature. This will disable the `ReadBytes` and
`WriteBytes` traits.

## License

[MIT](LICENSE-MIT)
