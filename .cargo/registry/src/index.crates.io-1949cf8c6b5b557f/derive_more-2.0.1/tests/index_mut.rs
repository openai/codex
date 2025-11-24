#![allow(dead_code)] // some code is tested for type checking only

use derive_more::IndexMut;

#[derive(IndexMut)]
struct MyVec(Vec<i32>);
// `Index` implementation is required for `IndexMut`.
impl<__IdxT> ::core::ops::Index<__IdxT> for MyVec
where
    Vec<i32>: ::core::ops::Index<__IdxT>,
{
    type Output = <Vec<i32> as ::core::ops::Index<__IdxT>>::Output;
    #[inline]
    fn index(&self, idx: __IdxT) -> &Self::Output {
        <Vec<i32> as ::core::ops::Index<__IdxT>>::index(&self.0, idx)
    }
}

#[derive(IndexMut)]
struct Numbers {
    #[index_mut]
    numbers: Vec<i32>,
    useless: bool,
}

// `Index` implementation is required for `IndexMut`.
impl<__IdxT> ::core::ops::Index<__IdxT> for Numbers
where
    Vec<i32>: ::core::ops::Index<__IdxT>,
{
    type Output = <Vec<i32> as ::core::ops::Index<__IdxT>>::Output;
    #[inline]
    fn index(&self, idx: __IdxT) -> &Self::Output {
        <Vec<i32> as ::core::ops::Index<__IdxT>>::index(&self.numbers, idx)
    }
}
