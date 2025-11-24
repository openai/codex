# What `#[derive(Not)]` generates

The derived `Not` implementation simply negates all the fields of a
struct and returns that as a new instance of the struct.
For enums all fields of the active variant of the enum are negated and a new
instance of the same variant with these negated fields is returned.




## Tuple structs

When deriving for a tuple struct with two fields like this:

```rust
# use derive_more::Not;
#
#[derive(Not)]
struct MyInts(i32, i32);
```

Code like this will be generated:

```rust
# struct MyInts(i32, i32);
impl derive_more::Not for MyInts {
    type Output = MyInts;
    fn not(self) -> MyInts {
        MyInts(self.0.not(), self.1.not())
    }
}
```

The behaviour is similar with more or less fields.




## Regular structs

When deriving for a regular struct with two fields like this:

```rust
# use derive_more::Not;
#
#[derive(Not)]
struct Point2D {
    x: i32,
    y: i32,
}
```

Code like this will be generated:

```rust
# struct Point2D {
#     x: i32,
#     y: i32,
# }
impl derive_more::Not for Point2D {
    type Output = Point2D;
    fn not(self) -> Point2D {
        Point2D {
            x: self.x.not(),
            y: self.y.not(),
        }
    }
}
```

The behaviour is similar with more or less fields.




## Enums

For each enum variant `Not` is derived in a similar way as it would be derived
if it would be its own type.
For instance when deriving `Not` for an enum like this:

```rust
# use derive_more::Not;
#
#[derive(Not)]
enum MixedInts {
    SmallInt(i32),
    BigInt(i64),
    TwoSmallInts(i32, i32),
    NamedSmallInts { x: i32, y: i32 },
    UnsignedOne(u32),
    UnsignedTwo(u32),
}
```

Code like this will be generated:

```rust
# enum MixedInts {
#     SmallInt(i32),
#     BigInt(i64),
#     TwoSmallInts(i32, i32),
#     NamedSmallInts { x: i32, y: i32 },
#     UnsignedOne(u32),
#     UnsignedTwo(u32),
# }
impl derive_more::Not for MixedInts {
    type Output = MixedInts;
    fn not(self) -> MixedInts {
        match self {
            MixedInts::SmallInt(__0) => MixedInts::SmallInt(__0.not()),
            MixedInts::BigInt(__0) => MixedInts::BigInt(__0.not()),
            MixedInts::TwoSmallInts(__0, __1) => MixedInts::TwoSmallInts(__0.not(), __1.not()),
            MixedInts::NamedSmallInts { x: __0, y: __1 } => {
                MixedInts::NamedSmallInts {
                    x: __0.not(),
                    y: __1.not(),
                }
            }
            MixedInts::UnsignedOne(__0) => MixedInts::UnsignedOne(__0.not()),
            MixedInts::UnsignedTwo(__0) => MixedInts::UnsignedTwo(__0.not()),
        }
    }
}
```

There is one important thing to remember though.
If you add a unit variant to the enum its return type will change from
`EnumType` to `Result<EnumType>`.
This is because Unit cannot have `Not` implemented.
So, when deriving `Not` for an enum like this:

```rust
# use derive_more::Not;
#
#[derive(Not)]
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
impl derive_more::Not for EnumWithUnit {
    type Output = Result<EnumWithUnit, derive_more::UnitError>;
    fn not(self) -> Result<EnumWithUnit, derive_more::UnitError> {
        match self {
            EnumWithUnit::SmallInt(__0) => Ok(EnumWithUnit::SmallInt(__0.not())),
            EnumWithUnit::Unit => Err(derive_more::UnitError::new("not")),
        }
    }
}
```
