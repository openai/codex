#![allow(dead_code)]

use std::ops::Add;

use darling::{FromDeriveInput, FromMeta};

#[derive(Debug, Clone, FromMeta)]
#[darling(bound = "T: FromMeta + Add")]
struct Wrapper<T>(pub T);

impl<T: Add> Add for Wrapper<T> {
    type Output = Wrapper<<T as Add>::Output>;
    fn add(self, rhs: Self) -> Wrapper<<T as Add>::Output> {
        Wrapper(self.0 + rhs.0)
    }
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(hello), bound = "Wrapper<T>: Add, T: FromMeta")]
struct Foo<T> {
    lorem: Wrapper<T>,
}

#[test]
fn expansion() {}
