# What `#[derive(IndexMut)]` generates

Deriving `IndexMut` only works for a single field of a struct.
Furthermore it requires that the type also implements `Index`, so usually
`Index` should also be derived.
The result is that you will mutably index it's member directly.

With `#[index_mut]` or `#[index_mut(ignore)]` it's possible to indicate the
field that you want to derive `IndexMut` for.




## Example usage

```rust
# use derive_more::{Index, IndexMut};
#
#[derive(Index, IndexMut)]
struct MyVec(Vec<i32>);

#[derive(Index, IndexMut)]
struct Numbers {
    #[index]
    #[index_mut]
    numbers: Vec<i32>,
    useless: bool,
}

let mut myvec = MyVec(vec![5, 8]);
myvec[0] = 50;
assert_eq!(50, myvec[0]);

let mut numbers = Numbers{numbers: vec![100, 200], useless: false};
numbers[1] = 400;
assert_eq!(400, numbers[1]);
```




## Regular structs

When deriving `IndexMut` for a struct:

```rust
# use derive_more::{Index, IndexMut};
#
#[derive(Index, IndexMut)]
struct Numbers {
    #[index]
    #[index_mut]
    numbers: Vec<i32>,
    useless: bool,
}
```

Code like this will be generated to implement `IndexMut`:

```rust
# use ::core::ops::Index;
# struct Numbers {
#     numbers: Vec<i32>,
#     useless: bool,
# }
# impl<__IdxT> Index<__IdxT> for Numbers
# where
#     Vec<i32>: Index<__IdxT>,
# {
#     type Output = <Vec<i32> as Index<__IdxT>>::Output;
#     #[inline]
#     fn index(&self, idx: __IdxT) -> &Self::Output {
#         <Vec<i32> as Index<__IdxT>>::index(&self.numbers, idx)
#     }
# }
impl<__IdxT> derive_more::IndexMut<__IdxT> for Numbers
where
    Vec<i32>: derive_more::IndexMut<__IdxT>,
{
    #[inline]
    fn index_mut(&mut self, idx: __IdxT) -> &mut Self::Output {
        <Vec<i32> as derive_more::IndexMut<__IdxT>>::index_mut(&mut self.numbers, idx)
    }
}
```




## Enums

Deriving `IndexMut` is not supported for enums.
