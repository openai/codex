#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Debug, Derivative, PartialEq)]
#[derivative(Default="new")]
struct Foo {
    foo: u8,
    #[derivative(Default(value="42"))]
    bar: u8,
}

#[derive(Debug, Derivative, PartialEq)]
#[derivative(Default(new="true"))]
struct Bar (
    u8,
    #[derivative(Default(value="42"))]
    u8,
);

#[derive(Debug, PartialEq)]
struct B1(u8, u8);
#[derive(Debug, PartialEq)]
struct B2{a:u8, b:u8}

#[derive(Debug, Derivative, PartialEq)]
#[derivative(Default(new="true"))]
struct Baz (
    #[derivative(Default(value="[1,2]"))]
    [u8;2],
    #[derivative(Default(value="[3;2]"))]
    [u8;2],
    #[derivative(Default(value="(4,5)"))]
    (u8, u8),
    #[derivative(Default(value="B1(6,7)"))]
    B1,
    #[derivative(Default(value="B2{a:8,b:9}"))]
    B2,
);

#[derive(Debug, Derivative, PartialEq)]
#[derivative(Default)]
enum Enum1 {
    #[allow(dead_code)]
    A,
    #[derivative(Default)]
    B,
}

#[derive(Debug, Derivative, PartialEq)]
#[derivative(Default)]
enum Enum2 {
    #[derivative(Default)]
    A,
    #[allow(dead_code)]
    B,
}

#[derive(Debug, Derivative, PartialEq)]
#[derivative(Default)]
struct A(#[derivative(Default(value="NoDefault"))] NoDefault);

#[derive(Debug, PartialEq)]
struct NoDefault;

#[test]
fn main() {
    assert_eq!(Foo::default(), Foo { foo: 0, bar: 42 });
    assert_eq!(Foo::new(), Foo { foo: 0, bar: 42 });
    assert_eq!(Bar::default(), Bar(0, 42));
    assert_eq!(Bar::new(), Bar(0, 42));
    assert_eq!(Baz::new(), Baz([1,2], [3,3], (4,5), B1(6,7), B2{a:8,b:9}));
    assert_eq!(A::default(), A(NoDefault));
    assert_eq!(Enum1::default(), Enum1::B);
    assert_eq!(Enum2::default(), Enum2::A);
}
