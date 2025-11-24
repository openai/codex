# What `#[derive(TryInto)]` generates

This derive allows you to convert enum variants into their corresponding
variant types.
One thing to note is that this derive doesn't actually generate an
implementation for the `TryInto` trait.
Instead it derives `TryFrom` for each variant in the enum and thus has an
indirect implementation of `TryInto` as recommended by the
[docs](https://doc.rust-lang.org/core/convert/trait.TryInto.html).

By using `#[try_into(owned, ref, ref_mut)]` it's possible to derive a `TryInto`
implementation for reference types as well.
You can pick any combination of `owned`, `ref` and `ref_mut`.
If that's not provided the default is `#[try_into(owned)]`.

With `#[try_into]` or `#[try_into(ignore)]` it's possible to indicate which
variants you want to derive `TryInto` for.




## Example usage

```rust
# use derive_more::TryInto;
#
#[derive(TryInto, Clone, Debug)]
#[try_into(owned, ref, ref_mut)]
enum MixedData {
    Int(u32),
    String(String),
}

let mixed_string = MixedData::String("foo".to_string());
let mixed_int1 = MixedData::Int(123);
let mixed_int2 = mixed_int1.clone();
let mut mixed_int3 = mixed_int1.clone();

assert_eq!(123u32, mixed_int1.try_into().unwrap());

let int_ref : &u32 = (&mixed_int2).try_into().unwrap();
assert_eq!(&123u32, int_ref);

let int_ref_mut : &mut u32 = (&mut mixed_int3).try_into().unwrap();
assert_eq!(&mut 123u32, int_ref_mut);

assert_eq!("foo".to_string(), String::try_from(mixed_string.clone()).unwrap());

assert!(u32::try_from(mixed_string).is_err());
```




## Structs

Deriving `TryInto` for structs is not supported because there is no failing
mode. Use `#[derive(Into)]` instead. `TryInto` will automatically get a
blanket implementation through `TryFrom`, automatically derived from `From`,
which `#[derive(Into)]` produces.




## Enums

When deriving `TryInto` for an enum, each enum variant gets its own
`TryFrom` implementation.
For instance, when deriving `TryInto` for an enum link this:

```rust
# use derive_more::TryInto;
#
#[derive(TryInto)]
enum MixedInts {
    SmallInt(i32),
    BigInt(i64),
    TwoSmallInts(i32, i32),
    NamedSmallInts { x: i64, y: i64 },
    UnsignedOne(u32),
    UnsignedTwo(u32),
    #[try_into(ignore)]
    NotImportant,
}
```

Code like this will be generated:

```rust
# enum MixedInts {
#     SmallInt(i32),
#     BigInt(i64),
#     TwoSmallInts(i32, i32),
#     NamedSmallInts { x: i64, y: i64 },
#     UnsignedOne(u32),
#     UnsignedTwo(u32),
# }
impl TryFrom<MixedInts> for (i32) {
    type Error = derive_more::TryIntoError<MixedInts>;
    fn try_from(value: MixedInts) -> Result<Self, derive_more::TryIntoError<MixedInts>> {
        match value {
            MixedInts::SmallInt(__0) => Ok(__0),
            _ => Err(derive_more::TryIntoError::new(value, "SmallInt", "i32")),
        }
    }
}
impl TryFrom<MixedInts> for (i64) {
    type Error = derive_more::TryIntoError<MixedInts>;
    fn try_from(value: MixedInts) -> Result<Self, derive_more::TryIntoError<MixedInts>> {
        match value {
            MixedInts::BigInt(__0) => Ok(__0),
            _ => Err(derive_more::TryIntoError::new(value, "BigInt", "i64")),
        }
    }
}
impl TryFrom<MixedInts> for (i32, i32) {
    type Error = derive_more::TryIntoError<MixedInts>;
    fn try_from(value: MixedInts) -> Result<Self, derive_more::TryIntoError<MixedInts>> {
        match value {
            MixedInts::TwoSmallInts(__0, __1) => Ok((__0, __1)),
            _ => Err(derive_more::TryIntoError::new(value, "TwoSmallInts", "(i32, i32)")),
        }
    }
}
impl TryFrom<MixedInts> for (i64, i64) {
    type Error = derive_more::TryIntoError<MixedInts>;
    fn try_from(value: MixedInts) -> Result<Self, derive_more::TryIntoError<MixedInts>> {
        match value {
            MixedInts::NamedSmallInts { x: __0, y: __1 } => Ok((__0, __1)),
            _ => Err(derive_more::TryIntoError::new(value, "NamedSmallInts", "(i64, i64)")),
        }
    }
}
impl TryFrom<MixedInts> for (u32) {
    type Error = derive_more::TryIntoError<MixedInts>;
    fn try_from(value: MixedInts) -> Result<Self, derive_more::TryIntoError<MixedInts>> {
        match value {
            MixedInts::UnsignedOne(__0) | MixedInts::UnsignedTwo(__0) => Ok(__0),
            _ => Err(derive_more::TryIntoError::new(value, "UnsignedOne", "u32")),
        }
    }
}
```

When deriving `TryInto` for an enum with Unit variants like this:

```rust
# use derive_more::TryInto;
#
#[derive(TryInto)]
enum EnumWithUnit {
    SmallInt(i32),
    Unit,
}
```

Code like this will be generated:

```rust
# enum EnumWithUnit {
#     SmallInt(i32),
#     Unit,
# }
impl TryFrom<EnumWithUnit> for (i32) {
    type Error = derive_more::TryIntoError<EnumWithUnit>;
    fn try_from(value: EnumWithUnit) -> Result<Self, derive_more::TryIntoError<EnumWithUnit>> {
        match value {
            EnumWithUnit::SmallInt(__0) => Ok(__0),
            _ => Err(derive_more::TryIntoError::new(value, "SmallInt", "i32")),
        }
    }
}
impl TryFrom<EnumWithUnit> for () {
    type Error = derive_more::TryIntoError<EnumWithUnit>;
    fn try_from(value: EnumWithUnit) -> Result<Self, derive_more::TryIntoError<EnumWithUnit>> {
        match value {
            EnumWithUnit::Unit => Ok(()),
            _ => Err(derive_more::TryIntoError::new(value, "Unit", "()")),
        }
    }
}
```
