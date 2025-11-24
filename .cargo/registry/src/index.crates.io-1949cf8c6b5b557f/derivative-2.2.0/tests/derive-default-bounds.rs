#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Debug, Derivative, PartialEq)]
#[derivative(Default="new")]
struct Foo<T, U> {
    foo: T,
    #[derivative(Default(value="min()", bound="U: std::ops::Not<Output=U>, U: Default"))]
    bar: U,
}

#[derive(Debug, Derivative, PartialEq)]
#[derivative(Default(bound="T: Default, U: std::ops::Not<Output=U>, U: Default", new="true"))]
struct Bar<T, U> {
    foo: T,
    #[derivative(Default(value="min()"))]
    bar: U,
}

fn min<T: Default + std::ops::Not<Output=T>>() -> T {
    !T::default()
}

#[derive(Debug, Derivative, PartialEq)]
#[derivative(Default(bound=""))]
struct WithOption<T> {
    foo: Option<T>,
}

struct NonDefault;

#[test]
fn main() {
    assert_eq!(Foo::default(), Foo { foo: 0u8, bar: 0xffu8 });
    assert_eq!(Bar::default(), Bar { foo: 0u8, bar: 0xffu8 });
    assert_eq!(Foo::new(), Foo { foo: 0u8, bar: 0xffu8 });
    assert_eq!(Bar::new(), Bar { foo: 0u8, bar: 0xffu8 });
    WithOption::<NonDefault>::default();
}
