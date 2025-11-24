# What `#[derive(Index)]` generates

Deriving `Index` only works for a single field of a struct.
The result is that you will index it's member directly.

With `#[index]` or `#[index(ignore)]` it's possible to indicate the field that
you want to derive `Index` for.




## Example usage

```rust
# use derive_more::Index;
#
#[derive(Index)]
struct MyVec(Vec<i32>);

// You can specify the field you want to derive Index for
#[derive(Index)]
struct Numbers {
    #[index]
    numbers: Vec<i32>,
    useless: bool,
}

assert_eq!(5, MyVec(vec![5, 8])[0]);
assert_eq!(200, Numbers { numbers: vec![100, 200], useless: false }[1]);
```




## Structs

When deriving `Index` for a struct:

```rust
# use derive_more::Index;
#
#[derive(Index)]
struct Numbers {
    #[index]
    numbers: Vec<i32>,
    useless: bool,
}
```

Code like this will be generated:

```rust
# struct Numbers {
#     numbers: Vec<i32>,
#     useless: bool,
# }
impl<__IdxT> derive_more::Index<__IdxT> for Numbers
where
    Vec<i32>: derive_more::Index<__IdxT>,
{
    type Output = <Vec<i32> as derive_more::Index<__IdxT>>::Output;
    #[inline]
    fn index(&self, idx: __IdxT) -> &Self::Output {
        <Vec<i32> as derive_more::Index<__IdxT>>::index(&self.numbers, idx)
    }
}
```




## Enums

Deriving `Index` is not supported for enums.
