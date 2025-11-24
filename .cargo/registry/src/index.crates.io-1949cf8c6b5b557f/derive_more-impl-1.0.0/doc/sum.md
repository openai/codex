# Using `#[derive(Sum)]`

The derived `Sum` implementation will allow an iterator of your type to be
summed together into a new instance of the type with all the fields added
together. Apart from the original types requiring an implementation of `Sum`, it
is also required that your type to implements `Add`. So normally you want to
derive that one as well.

All this is also true for the `Product`, except that then all the fields are
multiplied and an implementation of `Mul` is required. This is usually the
easiest to implement by adding `#[derive(MulSelf)]`.




## Example usage

```rust
# use derive_more::{Add, Sum};
#
#[derive(Add, Sum, PartialEq)]
struct MyInts(i32, i64);

let int_vec = vec![MyInts(2, 3), MyInts(4, 5), MyInts(6, 7)];
assert!(MyInts(12, 15) == int_vec.into_iter().sum())
```




## Structs

When deriving `Sum` for a struct with two fields its like this:

```rust
# use derive_more::{Add, Sum};
#
#[derive(Add, Sum)]
struct MyInts(i32, i64);
```

Code like this will be generated for the `Sum` implementation:

```rust
# use ::core::ops::Add;
# struct MyInts(i32, i64);
# impl Add for MyInts {
#     type Output = MyInts;
#     #[inline]
#     fn add(self, rhs: MyInts) -> MyInts {
#         MyInts(self.0.add(rhs.0), self.1.add(rhs.1))
#     }
# }
impl derive_more::Sum for MyInts {
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(
            MyInts(
                derive_more::core::iter::empty::<i32>().sum(),
                derive_more::core::iter::empty::<i64>().sum(),
            ),
            derive_more::core::ops::Add::add,
        )
    }
}
```

The trick here is that we get the identity struct by calling sum on empty
iterators.
This way we can get the identity for sum (i.e. `0`) and the identity for product
(i.e. `1`).




## Enums

Deriving `Sum` for enums is not supported.
