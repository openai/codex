#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(Debug)]
#[repr(C, packed)]
struct Foo {
    foo: u8,
    #[derivative(Debug="ignore")]
    bar: u8,
}

#[derive(Derivative)]
#[derivative(Debug)]
#[repr(C, packed)]
struct Bar (
    u8,
    #[derivative(Debug="ignore")]
    u8,
);

#[derive(Derivative)]
#[derivative(Debug)]
#[repr(C, packed)]
struct F(#[derivative(Debug="ignore")] isize);

#[derive(Derivative)]
#[derivative(Debug)]
#[repr(C, packed)]
struct G(isize, #[derivative(Debug="ignore")] isize);

#[derive(Derivative)]
#[derivative(Debug)]
#[repr(C, packed)]
struct J(#[derivative(Debug="ignore")] NoDebug);

#[derive(Derivative)]
#[derivative(Debug)]
#[repr(C, packed)]
struct K(isize, #[derivative(Debug="ignore")] NoDebug);

#[derive(Derivative)]
#[derivative(Debug)]
#[repr(C, packed)]
struct L {
    #[derivative(Debug="ignore")]
    foo: NoDebug
}

struct NoDebug;

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
    assert_eq!(Foo { foo: 42, bar: 1 }.to_show(), "Foo { foo: 42 }".to_string());
    assert_eq!(Bar(42, 1).to_show(), "Bar(42)".to_string());
    assert_eq!(F(42).to_show(), "F".to_string());
    assert_eq!(G(42, 0).to_show(), "G(42)".to_string());
    assert_eq!(J(NoDebug).to_show(), "J".to_string());
    assert_eq!(K(42, NoDebug).to_show(), "K(42)".to_string());
    assert_eq!(L{ foo: NoDebug }.to_show(), "L".to_string());
}
