# What `#[derive(Mul)]` generates

Deriving `Mul` is quite different from deriving `Add`. It is not used to
multiply two structs together. Instead it will normally multiply a struct, which
can have multiple fields, with a single primitive type (e.g. a `u64`). A new
struct is then created with all the fields from the previous struct multiplied
by this other value.

A simple way of explaining the reasoning behind this difference between `Add`
and `Mul` deriving, is looking at arithmetic on meters.
One meter can be added to one meter, to get two meters. Also, one meter times
two would be two meters, but one meter times one meter would be one square meter.
As this second case clearly requires more knowledge about the meaning of the
type in question deriving for this is not implemented.

NOTE: In case you don't want this behaviour you can add `#[mul(forward)]` in
addition to `#[derive(Mul)]`. This will instead generate a `Mul` implementation
with the same semantics as `Add`.




## Tuple structs

When deriving for a tuple struct with a single field (i.e. a newtype) like this:

```rust
# use derive_more::Mul;
#
#[derive(Mul)]
struct MyInt(i32);
```

Code like this will be generated:

```rust
# struct MyInt(i32);
impl<__RhsT> derive_more::Mul<__RhsT> for MyInt
    where i32: derive_more::Mul<__RhsT, Output = i32>
{
    type Output = MyInt;
    fn mul(self, rhs: __RhsT) -> MyInt {
        MyInt(self.0.mul(rhs))
    }
}
```

The behaviour is slightly different for multiple fields, since the right hand
side of the multiplication now needs the `Copy` trait.
For instance when deriving for a tuple struct with two fields like this:

```rust
# use derive_more::Mul;
#
#[derive(Mul)]
struct MyInts(i32, i32);
```

Code like this will be generated:

```rust
# struct MyInts(i32, i32);
impl<__RhsT: Copy> derive_more::Mul<__RhsT> for MyInts
    where i32: derive_more::Mul<__RhsT, Output = i32>
{
    type Output = MyInts;
    fn mul(self, rhs: __RhsT) -> MyInts {
        MyInts(self.0.mul(rhs), self.1.mul(rhs))
    }
}
```

The behaviour is similar with more or less fields.




## Regular structs

When deriving `Mul` for a regular struct with a single field like this:

```rust
# use derive_more::Mul;
#
#[derive(Mul)]
struct Point1D {
    x: i32,
}
```

Code like this will be generated:

```rust
# struct Point1D {
#     x: i32,
# }
impl<__RhsT> derive_more::Mul<__RhsT> for Point1D
    where i32: derive_more::Mul<__RhsT, Output = i32>
{
    type Output = Point1D;
    fn mul(self, rhs: __RhsT) -> Point1D {
        Point1D { x: self.x.mul(rhs) }
    }
}
```

The behaviour is again slightly different when deriving for a struct with multiple
fields, because it still needs the `Copy` as well.
For instance when deriving for a tuple struct with two fields like this:

```rust
# use derive_more::Mul;
#
#[derive(Mul)]
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
impl<__RhsT: Copy> derive_more::Mul<__RhsT> for Point2D
    where i32: derive_more::Mul<__RhsT, Output = i32>
{
    type Output = Point2D;
    fn mul(self, rhs: __RhsT) -> Point2D {
        Point2D {
            x: self.x.mul(rhs),
            y: self.y.mul(rhs),
        }
    }
}
```




## Enums

Deriving `Mul` for enums is not (yet) supported, except when you use
`#[mul(forward)]`.
Although it shouldn't be impossible no effort has been put into this yet.
