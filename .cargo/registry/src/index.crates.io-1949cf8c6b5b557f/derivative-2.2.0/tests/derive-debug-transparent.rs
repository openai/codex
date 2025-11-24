#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(Debug="transparent")]
struct A(isize);

#[derive(Derivative)]
#[derivative(Debug="transparent")]
struct B([isize; 1]);

#[derive(Derivative)]
#[derivative(Debug)]
enum C {
    Foo(u8),
    #[derivative(Debug="transparent")]
    Bar(u8),
}

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
    assert_eq!(A(42).to_show(), "42".to_string());
    assert_eq!(B([42]).to_show(), "[42]".to_string());
    assert_eq!(C::Foo(42).to_show(), "Foo(42)".to_string());
    assert_eq!(C::Bar(42).to_show(), "42".to_string());
}
