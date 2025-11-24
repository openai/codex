#![allow(clippy::blacklisted_name, clippy::redundant_clone, clippy::trivially_copy_pass_by_ref)]

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Debug, Derivative, PartialEq)]
#[derivative(Clone)]
struct Foo {
    foo: u8,
    #[derivative(Clone(clone_with="seventh"))]
    bar: u8,
}

fn seventh(a: &u8) -> u8 {
    a/7
}

#[derive(Debug, PartialEq)]
struct EvilCloneFrom(u8);

impl Clone for EvilCloneFrom {
    fn clone(&self) -> Self {
        EvilCloneFrom(self.0)
    }

    fn clone_from(&mut self, _: &Self) {
        self.0 = 42;
    }
}

#[derive(Derivative)]
#[derivative(Clone(clone_from="true"))]
struct StructWithCloneFrom(EvilCloneFrom);

#[derive(Debug, Derivative, PartialEq)]
#[derivative(Clone(clone_from="true"))]
enum EnumWithCloneFrom {
    Evil(EvilCloneFrom),
    Good(u32),
    None
}

#[test]
fn main() {
    let foo = Foo { foo: 31, bar: 42 };
    assert_eq!(Foo { foo: 31, bar: 6 }, foo.clone());

    let mut foo = StructWithCloneFrom(EvilCloneFrom(27));
    foo.clone_from(&StructWithCloneFrom(EvilCloneFrom(0)));
    assert_eq!((foo.0).0, 42);

    let mut foo = EnumWithCloneFrom::Evil(EvilCloneFrom(27));
    foo.clone_from(&EnumWithCloneFrom::Evil(EvilCloneFrom(0)));
    assert_eq!(foo, EnumWithCloneFrom::Evil(EvilCloneFrom(42)));

    let mut foo = EnumWithCloneFrom::Evil(EvilCloneFrom(27));
    foo.clone_from(&EnumWithCloneFrom::None);
    assert_eq!(foo, EnumWithCloneFrom::None);

    let mut foo = EnumWithCloneFrom::Good(27);
    foo.clone_from(&EnumWithCloneFrom::None);
    assert_eq!(foo, EnumWithCloneFrom::None);
}
