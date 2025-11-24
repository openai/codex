bitflags
========

[![Rust](https://github.com/bitflags/bitflags/workflows/Rust/badge.svg)](https://github.com/bitflags/bitflags/actions)
[![Latest version](https://img.shields.io/crates/v/bitflags.svg)](https://crates.io/crates/bitflags)
[![Documentation](https://docs.rs/bitflags/badge.svg)](https://docs.rs/bitflags)
![License](https://img.shields.io/crates/l/bitflags.svg)

`bitflags` generates flags enums with well-defined semantics and ergonomic end-user APIs.

You can use `bitflags` to:

- provide more user-friendly bindings to C APIs where flags may or may not be fully known in advance.
- generate efficient options types with string parsing and formatting support.

You can't use `bitflags` to:

- guarantee only bits corresponding to defined flags will ever be set. `bitflags` allows access to the underlying bits type so arbitrary bits may be set.
- define bitfields. `bitflags` only generates types where set bits denote the presence of some combination of flags.

- [Documentation](https://docs.rs/bitflags)
- [Specification](https://github.com/bitflags/bitflags/blob/main/spec.md)
- [Release notes](https://github.com/bitflags/bitflags/releases)

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
bitflags = "2.10.0"
```

and this to your source code:

```rust
use bitflags::bitflags;
```

## Example

Generate a flags structure:

```rust
use bitflags::bitflags;

// The `bitflags!` macro generates `struct`s that manage a set of flags.
bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Flags: u32 {
        /// The value `A`, at bit position `0`.
        const A = 0b00000001;
        /// The value `B`, at bit position `1`.
        const B = 0b00000010;
        /// The value `C`, at bit position `2`.
        const C = 0b00000100;

        /// The combination of `A`, `B`, and `C`.
        const ABC = Self::A.bits() | Self::B.bits() | Self::C.bits();
    }
}

fn main() {
    let e1 = Flags::A | Flags::C;
    let e2 = Flags::B | Flags::C;
    assert_eq!((e1 | e2), Flags::ABC);   // union
    assert_eq!((e1 & e2), Flags::C);     // intersection
    assert_eq!((e1 - e2), Flags::A);     // set difference
    assert_eq!(!e2, Flags::A);           // set complement
}
```

## Cargo features

The `bitflags` library defines a few Cargo features that you can opt-in to:

- `std`: Implement the `Error` trait on error types used by `bitflags`.
- `serde`: Support deriving `serde` traits on generated flags types.
- `arbitrary`: Support deriving `arbitrary` traits on generated flags types.
- `bytemuck`: Support deriving `bytemuck` traits on generated flags types.

Also see [`bitflags_derive`](https://github.com/bitflags/bitflags-derive) for other flags-aware traits.

## Rust Version Support

The minimum supported Rust version is documented in the `Cargo.toml` file.
This may be bumped in minor releases as necessary.
