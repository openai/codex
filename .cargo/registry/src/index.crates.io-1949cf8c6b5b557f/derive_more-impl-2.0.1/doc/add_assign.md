# What `#[derive(AddAssign)]` generates

This code is very similar to the code that is generated for `#[derive(Add)]`.
The difference is that it mutates the existing instance instead of creating a
new one.




## Tuple structs

When deriving `AddAssign` for a tuple struct with two fields like this:

```rust
# use derive_more::AddAssign;
#
#[derive(AddAssign)]
struct MyInts(i32, i32);
```

Code like this will be generated:

```rust
# struct MyInts(i32, i32);
impl derive_more::core::ops::AddAssign for MyInts {
    fn add_assign(&mut self, rhs: MyInts) {
        self.0.add_assign(rhs.0);
        self.1.add_assign(rhs.1);
    }
}
```

The behaviour is similar with more or less fields.




## Regular structs

When deriving for a regular struct with two fields like this:

```rust
# use derive_more::AddAssign;
#
#[derive(AddAssign)]
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
impl derive_more::core::ops::AddAssign for Point2D {
    fn add_assign(&mut self, rhs: Point2D) {
        self.x.add_assign(rhs.x);
        self.y.add_assign(rhs.y);
    }
}
```

The behaviour is similar with more or less fields.




## Enums

Deriving `AddAssign` is not (yet) supported for enums.
This is mostly due to the fact that it is not trivial convert the `Add`
derivation code, because that returns a `Result<EnumType>` instead of an
`EnumType`.
Handling the case where it errors would be hard and maybe impossible.
