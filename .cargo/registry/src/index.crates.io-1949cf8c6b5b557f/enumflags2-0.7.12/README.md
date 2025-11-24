[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![LICENSE](https://img.shields.io/badge/license-apache-blue.svg)](LICENSE-APACHE)
[![Documentation](https://docs.rs/enumflags2/badge.svg)](https://docs.rs/enumflags2)
[![Crates.io Version](https://img.shields.io/crates/v/enumflags2.svg)](https://crates.io/crates/enumflags2)

# Enumflags

`enumflags2` implements the classic bitflags datastructure. Annotate an enum
with `#[bitflags]`, and `BitFlags<YourEnum>` will be able to hold arbitrary combinations
of your enum within the space of a single integer.

Unlike other crates, `enumflags2` makes the type-level distinction between
a single flag (`YourEnum`) and a set of flags (`BitFlags<YourEnum>`).
This allows idiomatic handling of bitflags, such as with `match` and `iter`.

## Features

- [x] Uses enums to represent individual flags&mdash;a set of flags is a separate type from a single flag.
- [x] Automatically chooses a free bit when you don't specify.
- [x] Detects incorrect BitFlags at compile time.
- [x] Has a similar API compared to the popular [bitflags](https://crates.io/crates/bitflags) crate.
- [x] Does not expose the generated types explicity. The user interacts exclusively with `struct BitFlags<Enum>;`.
- [x] The debug formatter prints the binary flag value as well as the flag enums: `BitFlags(0b1111, [A, B, C, D])`.
- [x] Optional support for serialization with the [`serde`](https://serde.rs/) feature flag.

## Example

```rust
use enumflags2::{bitflags, make_bitflags, BitFlags};

#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
enum Test {
    A = 0b0001,
    B = 0b0010,
    C, // unspecified variants pick unused bits automatically
    D = 0b1000,
}

// Flags can be combined with |, this creates a BitFlags of your type:
let a_b: BitFlags<Test> = Test::A | Test::B;
let a_c = Test::A | Test::C;
let b_c_d = make_bitflags!(Test::{B | C | D});

// The debug output lets you inspect both the numeric value and
// the actual flags:
assert_eq!(format!("{:?}", a_b), "BitFlags<Test>(0b11, A | B)");

// But if you'd rather see only one of those, that's available too:
assert_eq!(format!("{}", a_b), "A | B");
assert_eq!(format!("{:04b}", a_b), "0011");

// Iterate over the flags like a normal set
assert_eq!(a_b.iter().collect::<Vec<_>>(), &[Test::A, Test::B]);

// Query the contents with contains and intersects
assert!(a_b.contains(Test::A));
assert!(b_c_d.contains(Test::B | Test::C));
assert!(!(b_c_d.contains(a_b)));

assert!(a_b.intersects(a_c));
assert!(!(a_b.intersects(Test::C | Test::D)));
```

## Optional Feature Flags

- [`serde`](https://serde.rs/) implements `Serialize` and `Deserialize`
  for `BitFlags<T>`.
- `std` implements `std::error::Error` for `FromBitsError`.

## `const fn`-compatible APIs

**Background:** The subset of `const fn` features currently stabilized is pretty limited.
Most notably, [const traits are still at the RFC stage][const-trait-rfc],
which makes it impossible to use any overloaded operators in a const
context.

**Naming convention:** If a separate, more limited function is provided
for usage in a `const fn`, the name is suffixed with `_c`.

Apart from functions whose name ends with `_c`, the [`make_bitflags!`] macro
is often useful for many `const` and `const fn` usecases.

**Blanket implementations:** If you attempt to write a `const fn` ranging
over `T: BitFlag`, you will be met with an error explaining that currently,
the only allowed trait bound for a `const fn` is `?Sized`. You will probably
want to write a separate implementation for `BitFlags<T, u8>`,
`BitFlags<T, u16>`, etc — best accomplished by a simple macro.

**Documentation considerations:** The strategy described above is often used
by `enumflags2` itself. To avoid clutter in the auto-generated documentation,
the implementations for widths other than `u8` are marked with `#[doc(hidden)]`.

## Customizing `Default`

By default, creating an instance of `BitFlags<T>` with `Default` will result in an empty
set. If that's undesirable, you may customize this:

```rust
#[bitflags(default = B | C)]
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
enum Test {
    A = 0b0001,
    B = 0b0010,
    C = 0b0100,
    D = 0b1000,
}

assert_eq!(BitFlags::default(), Test::B | Test::C);
```

[const-trait-rfc]: https://github.com/rust-lang/rfcs/pull/2632
