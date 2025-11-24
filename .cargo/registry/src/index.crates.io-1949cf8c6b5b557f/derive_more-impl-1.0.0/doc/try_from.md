# What `#[derive(TryFrom)]` generates

Derive `TryFrom` allows you to convert enum discriminants into their corresponding variants.




## Enums

By default, a `TryFrom<isize>` is generated, matching the [type of the discriminant](https://doc.rust-lang.org/reference/items/enumerations.html#discriminants).
The type can be changed with a `#[repr(u/i*)]` attribute, e.g., `#[repr(u8)]` or `#[repr(i32)]`.
Only field-less variants can be constructed from their variant, therefore the `TryFrom` implementation will return an error for a discriminant representing a variant with fields.

```rust
# use derive_more::TryFrom;
#
#[derive(TryFrom, Debug, PartialEq)]
#[try_from(repr)]
#[repr(u32)]
enum Enum {
    ImplicitZero,
    ExplicitFive = 5,
    FieldSix(usize),
    EmptySeven{},
}

assert_eq!(Enum::ImplicitZero, Enum::try_from(0).unwrap());
assert_eq!(Enum::ExplicitFive, Enum::try_from(5).unwrap());
assert_eq!(Enum::EmptySeven{}, Enum::try_from(7).unwrap());

// Variants with fields are not supported, as the value for their fields would be undefined.
assert!(Enum::try_from(6).is_err());
```
