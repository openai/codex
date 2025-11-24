#![allow(unused)]

use ts_rs::TS;

#[test]
fn free() {
    assert_eq!(<[String]>::inline(), "Array<string>")
}

#[derive(TS)]
#[ts(export, export_to = "slices/")]
struct Interface {
    #[allow(dead_code)]
    a: [i32],
}

#[test]
fn interface() {
    assert_eq!(Interface::inline(), "{ a: Array<number>, }")
}

#[derive(TS)]
#[ts(export, export_to = "slices/")]
struct InterfaceRef<'a> {
    #[allow(dead_code)]
    a: &'a [&'a str],
}

#[test]
fn slice_ref() {
    assert_eq!(InterfaceRef::inline(), "{ a: Array<string>, }")
}

#[derive(TS)]
#[ts(export, export_to = "slices/")]
struct Newtype(#[allow(dead_code)] [i32]);

#[test]
fn newtype() {
    assert_eq!(Newtype::inline(), "Array<number>")
}

// Since slices usually need to be wrapped in a `Box` or other container,
// these tests should to check for that

#[test]
fn boxed_free() {
    assert_eq!(<Box<[String]>>::inline(), "Array<string>")
}

#[derive(TS)]
#[ts(export, export_to = "slices/")]
struct InterfaceBoxed {
    #[allow(dead_code)]
    a: Box<[i32]>,
}

#[test]
fn boxed_interface() {
    assert_eq!(InterfaceBoxed::inline(), "{ a: Array<number>, }")
}

#[derive(TS)]
#[ts(export, export_to = "slices/")]
struct NewtypeBoxed(#[allow(dead_code)] Box<[i32]>);

#[test]
fn boxed_newtype() {
    assert_eq!(NewtypeBoxed::inline(), "Array<number>")
}

#[derive(TS)]
#[ts(export, export_to = "slices/nested/")]
struct InnerMost;

#[derive(TS)]
#[ts(export, export_to = "slices/nested/")]
struct Nested<'a>(&'a [InnerMost]);
