# fixed_decimal [![crates.io](https://img.shields.io/crates/v/fixed_decimal)](https://crates.io/crates/fixed_decimal)

<!-- cargo-rdme start -->

`fixed_decimal` is a utility crate of the [`ICU4X`] project.

This crate provides [`Decimal`] and [`UnsignedDecimal`], essential APIs for representing numbers in a human-readable format.
These types are particularly useful for formatting and plural rule selection, and are optimized for operations on individual digits.

## Examples

```rust
use fixed_decimal::Decimal;

let mut dec = Decimal::from(250);
dec.multiply_pow10(-2);
assert_eq!("2.50", format!("{}", dec));

#[derive(Debug, PartialEq)]
struct MagnitudeAndDigit(i16, u8);

let digits: Vec<MagnitudeAndDigit> = dec
    .magnitude_range()
    .map(|m| MagnitudeAndDigit(m, dec.digit_at(m)))
    .collect();

assert_eq!(
    vec![
        MagnitudeAndDigit(-2, 0),
        MagnitudeAndDigit(-1, 5),
        MagnitudeAndDigit(0, 2)
    ],
    digits
);
```

[`ICU4X`]: ../icu/index.html

<!-- cargo-rdme end -->

## More Information

For more information on development, authorship, contributing etc. please visit [`ICU4X home page`](https://github.com/unicode-org/icu4x).
