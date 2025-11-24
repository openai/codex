#![allow(clippy::box_collection, clippy::enum_variant_names, dead_code)]
#![allow(dead_code)]

use std::{
    collections::{BTreeMap, HashSet},
    fmt::Debug,
    rc::Rc,
};

use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct Generic<T>
where
    T: TS,
{
    value: T,
    values: Vec<T>,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct GenericAutoBound<T> {
    value: T,
    values: Vec<T>,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct GenericAutoBound2<T>
where
    T: PartialEq,
{
    value: T,
    values: Vec<T>,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct Container {
    foo: Generic<u32>,
    bar: Box<HashSet<Generic<u32>>>,
    baz: Box<BTreeMap<String, Rc<Generic<String>>>>,
}

macro_rules! declare {
    ($(#[$meta:meta])* $name:ident { $($fident:ident: $t:ty),+ $(,)? }) => {
        $(#[$meta])*
        struct $name {
            $(pub $fident: $t),+
        }
    }
}

declare! {
    #[derive(TS)]
    #[ts(export, export_to = "generics/")]
    TypeGroup {
        foo: Vec<Container>,
    }
}

#[test]
fn test() {
    assert_eq!(
        TypeGroup::decl(),
        "type TypeGroup = { foo: Array<Container>, };",
    );

    assert_eq!(
        Generic::<()>::decl(),
        "type Generic<T> = { value: T, values: Array<T>, };"
    );

    assert_eq!(
        GenericAutoBound::<()>::decl(),
        "type GenericAutoBound<T> = { value: T, values: Array<T>, };"
    );

    assert_eq!(
        GenericAutoBound2::<()>::decl(),
        "type GenericAutoBound2<T> = { value: T, values: Array<T>, };"
    );

    assert_eq!(
        Container::decl(),
        "type Container = { foo: Generic<number>, bar: Array<Generic<number>>, baz: { [key in string]?: Generic<string> }, };"
    );
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
enum GenericEnum<A, B, C> {
    A(A),
    B(B, B, B),
    C(Vec<C>),
    D(Vec<Vec<Vec<A>>>),
    E { a: A, b: B, c: C },
    X(Vec<i32>),
    Y(i32),
    Z(Vec<Vec<i32>>),
}

#[test]
fn generic_enum() {
    assert_eq!(
        GenericEnum::<(), (), ()>::decl(),
        r#"type GenericEnum<A, B, C> = { "A": A } | { "B": [B, B, B] } | { "C": Array<C> } | { "D": Array<Array<Array<A>>> } | { "E": { a: A, b: B, c: C, } } | { "X": Array<number> } | { "Y": number } | { "Z": Array<Array<number>> };"#
    )
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct NewType<T>(Vec<Vec<T>>);

#[test]
fn generic_newtype() {
    assert_eq!(
        NewType::<()>::decl(),
        r#"type NewType<T> = Array<Array<T>>;"#
    );
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct Tuple<T>(T, Vec<T>, Vec<Vec<T>>);

#[test]
fn generic_tuple() {
    assert_eq!(
        Tuple::<()>::decl(),
        r#"type Tuple<T> = [T, Array<T>, Array<Array<T>>];"#
    );
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct Struct<T> {
    a: T,
    b: (T, T),
    c: (T, (T, T)),
    d: [T; 3],
    e: [(T, T); 3],
    f: Vec<T>,
    g: Vec<Vec<T>>,
    h: Vec<[(T, T); 3]>,
}

#[test]
fn generic_struct() {
    assert_eq!(
        Struct::<()>::decl(),
        "type Struct<T> = { a: T, b: [T, T], c: [T, [T, T]], d: [T, T, T], e: [[T, T], [T, T], [T, T]], f: Array<T>, g: Array<Array<T>>, h: Array<[[T, T], [T, T], [T, T]]>, };"
    )
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct GenericInline<T> {
    t: T,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct ContainerInline {
    g: GenericInline<String>,
    #[ts(inline)]
    gi: GenericInline<String>,
    #[ts(flatten)]
    t: GenericInline<Vec<String>>,
}

#[test]
fn inline() {
    assert_eq!(
        GenericInline::<()>::decl(),
        "type GenericInline<T> = { t: T, };"
    );
    assert_eq!(
        ContainerInline::decl(),
        "type ContainerInline = { g: GenericInline<string>, gi: { t: string, }, t: Array<string>, };"
    );
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct GenericWithBounds<T: ToString> {
    t: T,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct ContainerWithBounds {
    g: GenericWithBounds<String>,

    #[ts(inline)]
    gi: GenericWithBounds<String>,

    #[ts(flatten)]
    t: GenericWithBounds<u32>,
}

#[test]
fn inline_with_bounds() {
    assert_eq!(
        GenericWithBounds::<&'static str>::decl(),
        "type GenericWithBounds<T> = { t: T, };"
    );
    assert_eq!(
        ContainerWithBounds::decl(),
        "type ContainerWithBounds = { g: GenericWithBounds<string>, gi: { t: string, }, t: number, };"
    );
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct GenericWithDefault<T = String> {
    t: T,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct ContainerWithDefault {
    g: GenericWithDefault<String>,

    #[ts(inline)]
    gi: GenericWithDefault<String>,

    #[ts(flatten)]
    t: GenericWithDefault<u32>,
}

#[test]
fn inline_with_default() {
    assert_eq!(
        GenericWithDefault::<()>::decl(),
        "type GenericWithDefault<T = string> = { t: T, };"
    );
    assert_eq!(
        ContainerWithDefault::decl(),
        "type ContainerWithDefault = { g: GenericWithDefault<string>, gi: { t: string, }, t: number, };"
    );
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct ADefault<T = String> {
    t: T,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct BDefault<U = Option<ADefault<i32>>> {
    u: U,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct YDefault {
    a1: ADefault,
    a2: ADefault<i32>,
}

#[test]
fn default() {
    assert_eq!(
        ADefault::<()>::decl(),
        "type ADefault<T = string> = { t: T, };"
    );

    assert_eq!(
        BDefault::<()>::decl(),
        "type BDefault<U = ADefault<number> | null> = { u: U, };"
    );
    assert!(BDefault::<()>::dependencies()
        .iter()
        .any(|dep| dep.ts_name == "ADefault"));

    assert_eq!(
        YDefault::decl(),
        "type YDefault = { a1: ADefault<string>, a2: ADefault<number>, };"
    )
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct ATraitBounds<T: ToString = i32> {
    t: T,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct BTraitBounds<T: ToString + Debug + Clone + 'static>(T);

#[derive(TS)]
#[ts(export, export_to = "generics/")]
enum CTraitBounds<T: Copy + Clone + PartialEq, K: Copy + PartialOrd = i32> {
    A { t: T },
    B(T),
    C,
    D(T, K),
}

// Types with const generics can't be exported
#[derive(TS)]
struct DTraitBounds<T: ToString, const N: usize> {
    t: [T; N],
}

#[test]
fn trait_bounds() {
    assert_eq!(
        ATraitBounds::<i32>::decl(),
        "type ATraitBounds<T = number> = { t: T, };"
    );

    assert_eq!(
        BTraitBounds::<&'static str>::decl(),
        "type BTraitBounds<T> = T;"
    );

    assert_eq!(
        CTraitBounds::<&'static str, i32>::decl(),
        r#"type CTraitBounds<T, K = number> = { "A": { t: T, } } | { "B": T } | "C" | { "D": [T, K] };"#
    );

    let ty = format!(
        "type DTraitBounds<T> = {{ t: [{}], }};",
        "T, ".repeat(41).trim_end_matches(", ")
    );
    assert_eq!(DTraitBounds::<&str, 41>::decl(), ty)
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct T0<T> {
    t0: T,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct P0<T> {
    p0: T,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct T1<T> {
    t0: T,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct P1<T> {
    p0: T,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct Parent {
    a: T1<T0<u32>>,
    b: T1<P1<T0<P0<u32>>>>,
    c: T1<P1<()>>,
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct GenericParent<T> {
    a_t: T1<T0<T>>,
    b_t: T1<P1<T0<P0<T>>>>,
    c_t: T1<P1<T>>,
    a_null: T1<T0<()>>,
    b_null: T1<P1<T0<P0<()>>>>,
    c_null: T1<P1<()>>,
}

#[test]
fn deeply_nested() {
    assert_eq!(
        Parent::inline(),
        "{ a: T1<T0<number>>, b: T1<P1<T0<P0<number>>>>, c: T1<P1<null>>, }"
    );
    assert_eq!(
        GenericParent::<()>::decl(),
        "type GenericParent<T> = { \
            a_t: T1<T0<T>>, \
            b_t: T1<P1<T0<P0<T>>>>, \
            c_t: T1<P1<T>>, \
            a_null: T1<T0<null>>, \
            b_null: T1<P1<T0<P0<null>>>>, \
            c_null: T1<P1<null>>, \
         };"
    );
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct SomeType(String);

#[derive(TS)]
#[ts(export, export_to = "generics/")]
enum MyEnum<A, B> {
    VariantA(A),
    VariantB(B),
}

#[derive(TS)]
#[ts(export, export_to = "generics/")]
struct ParentEnum {
    e: MyEnum<i32, i32>,
    #[ts(inline)]
    e1: MyEnum<i32, SomeType>,
}

#[test]
fn inline_generic_enum() {
    // This fails!
    // The #[ts(inline)] seems to inline recursively, so not only the definition of `MyEnum`, but
    // also the definition of `SomeType`.
    assert_eq!(
        ParentEnum::decl(),
        "type ParentEnum = { \
            e: MyEnum<number, number>, \
            e1: { \"VariantA\": number } | { \"VariantB\": SomeType }, \
        };"
    );
}
