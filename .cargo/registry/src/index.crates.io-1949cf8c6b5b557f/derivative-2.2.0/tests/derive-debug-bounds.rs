#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

use std::fmt::{Formatter, Result as FmtResult};

#[derive(Derivative)]
#[derivative(Debug)]
struct Foo<T, U> {
    foo: T,
    #[derivative(Debug(format_with="MyDebug::my_fmt", bound="U: MyDebug"))]
    bar: U,
}

#[derive(Derivative)]
#[derivative(Debug(bound="T: std::fmt::Debug, U: MyDebug"))]
struct Foo2<T, U> {
    foo: T,
    #[derivative(Debug(format_with="MyDebug::my_fmt"))]
    bar: U,
}

#[derive(Derivative)]
#[derivative(Debug)]
struct Bar<T, U> (
    T,
    #[derivative(Debug(format_with="MyDebug::my_fmt", bound="U: MyDebug"))]
    U,
);

#[derive(Derivative)]
#[derivative(Debug(bound="T: std::fmt::Debug, U: MyDebug"))]
struct Bar2<T, U> (
    T,
    #[derivative(Debug(format_with="MyDebug::my_fmt"))]
    U,
);

struct NoDebug;

struct GenericNeedsNoDebug<T>(T);
impl<T> std::fmt::Debug for GenericNeedsNoDebug<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> FmtResult {
        f.write_str("GenericNeedsNoDebug")
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
struct TestUnneededBound<T>( // Test that we don't add T: Debug
    #[derivative(Debug(bound=""))] GenericNeedsNoDebug<T>,
);

trait MyDebug {
    fn my_fmt(&self, f: &mut Formatter) -> FmtResult {
        f.write_str("MyDebug")
    }
}

impl MyDebug for i32 { }
impl<'a, T> MyDebug for &'a T { }


trait ToDebug {
    fn to_show(&self) -> String;
}

impl<T: std::fmt::Debug> ToDebug for T {
    fn to_show(&self) -> String {
        format!("{:?}", self)
    }
}

#[test]
fn main() {
    assert_eq!(Foo { foo: 42, bar: 0 }.to_show(), "Foo { foo: 42, bar: MyDebug }".to_string());
    assert_eq!(Foo2 { foo: 42, bar: 0 }.to_show(), "Foo2 { foo: 42, bar: MyDebug }".to_string());
    assert_eq!(Bar(42, 0).to_show(), "Bar(42, MyDebug)".to_string());
    assert_eq!(Bar2(42, 0).to_show(), "Bar2(42, MyDebug)".to_string());
    assert_eq!(TestUnneededBound(GenericNeedsNoDebug(NoDebug)).to_show(), "TestUnneededBound(GenericNeedsNoDebug)".to_string());
}
