#![allow(dead_code, clippy::box_collection)]

use std::borrow::Cow;

use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "generic_fields/")]
struct Newtype(Vec<Cow<'static, i32>>);

#[test]
fn newtype() {
    assert_eq!(Newtype::inline(), "Array<number>");
}

#[derive(TS)]
#[ts(export, export_to = "generic_fields/")]
struct NewtypeNested(Vec<Vec<i32>>);

#[test]
fn newtype_nested() {
    assert_eq!(NewtypeNested::inline(), "Array<Array<number>>");
}

#[test]
fn alias() {
    type Alias = Vec<String>;
    assert_eq!(Alias::inline(), "Array<string>");
}

#[test]
fn alias_nested() {
    type Alias = Vec<Vec<String>>;
    assert_eq!(Alias::inline(), "Array<Array<string>>");
}

#[derive(TS)]
#[ts(export, export_to = "generic_fields/")]
struct Struct {
    a: Box<Vec<String>>,
    b: (Vec<String>, Vec<String>),
    c: [Vec<String>; 3],
}

#[test]
fn named() {
    assert_eq!(
        Struct::inline(),
        "{ a: Array<string>, b: [Array<string>, Array<string>], c: [Array<string>, Array<string>, Array<string>], }"
    );
}

#[derive(TS)]
#[ts(export, export_to = "generic_fields/")]
struct StructNested {
    a: Vec<Vec<String>>,
    b: (Vec<Vec<String>>, Vec<Vec<String>>),
    c: [Vec<Vec<String>>; 3],
}

#[test]
fn named_nested() {
    assert_eq!(StructNested::inline(), "{ a: Array<Array<string>>, b: [Array<Array<string>>, Array<Array<string>>], c: [Array<Array<string>>, Array<Array<string>>, Array<Array<string>>], }");
}

#[derive(TS)]
#[ts(export, export_to = "generic_fields/")]
struct Tuple(Vec<i32>, (Vec<i32>, Vec<i32>), [Vec<i32>; 3]);

#[test]
fn tuple() {
    assert_eq!(
        Tuple::inline(),
        "[Array<number>, [Array<number>, Array<number>], [Array<number>, Array<number>, Array<number>]]"
    );
}

#[derive(TS)]
#[ts(export, export_to = "generic_fields/")]
struct TupleNested(
    Vec<Vec<i32>>,
    (Vec<Vec<i32>>, Vec<Vec<i32>>),
    [Vec<Vec<i32>>; 3],
);

#[test]
fn tuple_nested() {
    assert_eq!(
        TupleNested::inline(),
        "[Array<Array<number>>, [Array<Array<number>>, Array<Array<number>>], [Array<Array<number>>, Array<Array<number>>, Array<Array<number>>]]"
    );
}
