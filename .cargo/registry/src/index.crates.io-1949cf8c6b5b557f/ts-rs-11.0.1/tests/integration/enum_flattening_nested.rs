#![allow(dead_code)]

#[cfg(feature = "serde-compat")]
use serde::Serialize;
use ts_rs::TS;

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[ts(export, export_to = "enum_flattening_nested/externally_tagged/")]
struct FooExternally {
    #[cfg_attr(feature = "serde-compat", serde(flatten))]
    #[cfg_attr(not(feature = "serde-compat"), ts(flatten))]
    baz: BarExternally,
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[ts(export, export_to = "enum_flattening_nested/externally_tagged/")]
enum BarExternally {
    Baz { a: i32, a2: String },
    Biz { b: bool },
    Buz { c: String, d: Option<i32> },
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[ts(export, export_to = "enum_flattening_nested/externally_tagged/")]
struct NestedExternally {
    #[cfg_attr(feature = "serde-compat", serde(flatten))]
    #[cfg_attr(not(feature = "serde-compat"), ts(flatten))]
    a: FooExternally,
    u: u32,
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[ts(export, export_to = "enum_flattening_nested/externally_tagged/")]
struct NestedExternallyLonely {
    #[cfg_attr(feature = "serde-compat", serde(flatten))]
    #[cfg_attr(not(feature = "serde-compat"), ts(flatten))]
    a: FooExternally,
}

#[test]
fn externally_tagged() {
    // Notice here that baz is the only field inside `FooExternally`, so the parenthesis
    // aren't needed
    assert_eq!(
        FooExternally::inline(),
        r#"{ "Baz": { a: number, a2: string, } } | { "Biz": { b: boolean, } } | { "Buz": { c: string, d: number | null, } }"#
    );

    // But when flattening, the parenthesis are needed due to type intesections
    assert_eq!(
        NestedExternally::inline(),
        r#"{ u: number, } & ({ "Baz": { a: number, a2: string, } } | { "Biz": { b: boolean, } } | { "Buz": { c: string, d: number | null, } })"#
    );

    // And here, they are, again, unecessary
    assert_eq!(
        NestedExternallyLonely::inline(),
        r#"{ "Baz": { a: number, a2: string, } } | { "Biz": { b: boolean, } } | { "Buz": { c: string, d: number | null, } }"#
    );
}

#[derive(TS)]
#[ts(export, export_to = "enum_flattening_nested/adjacently_tagged/")]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
struct FooAdjecently {
    #[cfg_attr(feature = "serde-compat", serde(flatten))]
    #[cfg_attr(not(feature = "serde-compat"), ts(flatten))]
    baz: BarAdjecently,
}

#[derive(TS)]
#[ts(export, export_to = "enum_flattening_nested/adjacently_tagged/")]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[cfg_attr(feature = "serde-compat", serde(tag = "type", content = "stuff"))]
#[cfg_attr(not(feature = "serde-compat"), ts(tag = "type", content = "stuff"))]
enum BarAdjecently {
    Baz {
        a: i32,
        a2: String,
    },
    Biz {
        b: bool,
    },

    #[cfg_attr(feature = "serde-compat", serde(untagged))]
    #[cfg_attr(not(feature = "serde-compat"), ts(untagged))]
    Buz {
        c: String,
        d: Option<i32>,
    },
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
struct NestedAdjecently {
    #[cfg_attr(feature = "serde-compat", serde(flatten))]
    #[cfg_attr(not(feature = "serde-compat"), ts(flatten))]
    a: FooAdjecently,
    u: u32,
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[ts(export, export_to = "enum_flattening_nested/externally_tagged/")]
struct NestedAdjecentlyLonely {
    #[cfg_attr(feature = "serde-compat", serde(flatten))]
    #[cfg_attr(not(feature = "serde-compat"), ts(flatten))]
    a: FooAdjecently,
}

#[test]
fn adjacently_tagged() {
    assert_eq!(
        FooAdjecently::inline(),
        r#"{ "type": "Baz", "stuff": { a: number, a2: string, } } | { "type": "Biz", "stuff": { b: boolean, } } | { c: string, d: number | null, }"#
    );

    assert_eq!(
        NestedAdjecently::inline(),
        r#"{ u: number, } & ({ "type": "Baz", "stuff": { a: number, a2: string, } } | { "type": "Biz", "stuff": { b: boolean, } } | { c: string, d: number | null, })"#
    );

    assert_eq!(
        NestedAdjecentlyLonely::inline(),
        r#"{ "type": "Baz", "stuff": { a: number, a2: string, } } | { "type": "Biz", "stuff": { b: boolean, } } | { c: string, d: number | null, }"#
    );
}

#[derive(TS)]
#[ts(export, export_to = "enum_flattening_nested/internally_tagged/")]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
struct FooInternally {
    #[cfg_attr(feature = "serde-compat", serde(flatten))]
    #[cfg_attr(not(feature = "serde-compat"), ts(flatten))]
    baz: BarInternally,
}

#[derive(TS)]
#[ts(export, export_to = "enum_flattening_nested/internally_tagged/")]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[cfg_attr(feature = "serde-compat", serde(tag = "type"))]
#[cfg_attr(not(feature = "serde-compat"), ts(tag = "type"))]
enum BarInternally {
    Baz { a: i32, a2: String },
    Biz { b: bool },
    Buz { c: String, d: Option<i32> },
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
struct NestedInternally {
    #[cfg_attr(feature = "serde-compat", serde(flatten))]
    #[cfg_attr(not(feature = "serde-compat"), ts(flatten))]
    a: FooInternally,
    u: u32,
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[ts(export, export_to = "enum_flattening_nested/externally_tagged/")]
struct NestedInternallyLonely {
    #[cfg_attr(feature = "serde-compat", serde(flatten))]
    #[cfg_attr(not(feature = "serde-compat"), ts(flatten))]
    a: FooInternally,
}

#[test]
fn internally_tagged() {
    assert_eq!(
        FooInternally::inline(),
        r#"{ "type": "Baz", a: number, a2: string, } | { "type": "Biz", b: boolean, } | { "type": "Buz", c: string, d: number | null, }"#
    );

    assert_eq!(
        NestedInternally::inline(),
        r#"{ u: number, } & ({ "type": "Baz", a: number, a2: string, } | { "type": "Biz", b: boolean, } | { "type": "Buz", c: string, d: number | null, })"#
    );

    assert_eq!(
        NestedInternallyLonely::inline(),
        r#"{ "type": "Baz", a: number, a2: string, } | { "type": "Biz", b: boolean, } | { "type": "Buz", c: string, d: number | null, }"#
    );
}

#[derive(TS)]
#[ts(export, export_to = "enum_flattening_nested/untagged/")]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
struct FooUntagged {
    #[cfg_attr(feature = "serde-compat", serde(flatten))]
    #[cfg_attr(not(feature = "serde-compat"), ts(flatten))]
    baz: BarUntagged,
}

#[derive(TS)]
#[ts(export, export_to = "enum_flattening_nested/untagged/")]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[cfg_attr(feature = "serde-compat", serde(untagged))]
#[cfg_attr(not(feature = "serde-compat"), ts(untagged))]
enum BarUntagged {
    Baz { a: i32, a2: String },
    Biz { b: bool },
    Buz { c: String },
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
struct NestedUntagged {
    #[cfg_attr(feature = "serde-compat", serde(flatten))]
    #[cfg_attr(not(feature = "serde-compat"), ts(flatten))]
    a: FooUntagged,
    u: u32,
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[ts(export, export_to = "enum_flattening_nested/externally_tagged/")]
struct NestedUntaggedLonely {
    #[cfg_attr(feature = "serde-compat", serde(flatten))]
    #[cfg_attr(not(feature = "serde-compat"), ts(flatten))]
    a: FooUntagged,
}

#[test]
fn untagged() {
    assert_eq!(
        FooUntagged::inline(),
        r#"{ a: number, a2: string, } | { b: boolean, } | { c: string, }"#
    );

    assert_eq!(
        NestedUntagged::inline(),
        r#"{ u: number, } & ({ a: number, a2: string, } | { b: boolean, } | { c: string, })"#
    );

    assert_eq!(
        NestedUntaggedLonely::inline(),
        r#"{ a: number, a2: string, } | { b: boolean, } | { c: string, }"#
    );
}
