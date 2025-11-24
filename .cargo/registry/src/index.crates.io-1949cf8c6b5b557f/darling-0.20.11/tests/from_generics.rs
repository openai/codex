//! Tests for `FromGenerics`, and - indirectly - `FromGenericParam`.
//! These tests assume `FromTypeParam` is working and only look at whether the wrappers for magic
//! fields are working as expected.

use darling::{
    ast::{self, GenericParamExt},
    util::{Ignored, WithOriginal},
    FromDeriveInput, FromTypeParam, Result,
};

#[derive(FromDeriveInput)]
#[darling(attributes(lorem))]
struct MyReceiver {
    pub generics: ast::Generics<ast::GenericParam<MyTypeParam>>,
}

#[derive(FromTypeParam)]
#[darling(attributes(lorem))]
struct MyTypeParam {
    pub ident: syn::Ident,
    #[darling(default)]
    pub foo: bool,
    pub bar: Option<String>,
}

fn fdi<T: FromDeriveInput>(src: &str) -> Result<T> {
    FromDeriveInput::from_derive_input(&syn::parse_str(src).expect("Source parses"))
}

/// Verify that `ast::Generics` is populated correctly when there is no generics declaration
#[test]
fn no_generics() {
    let rec: MyReceiver = fdi("struct Baz;").expect("Input is well-formed");
    assert!(rec.generics.where_clause.is_none());
    assert_eq!(rec.generics.params.len(), 0);
}

#[test]
#[allow(clippy::bool_assert_comparison)]
fn expand_some() {
    let rec: MyReceiver = fdi(r#"
        struct Baz<
            'a,
            #[lorem(foo)] T,
            #[lorem(bar = "x")] U: Eq + ?Sized
        >(&'a T, U);
    "#)
    .expect("Input is well-formed");
    assert!(rec.generics.where_clause.is_none());

    // Make sure we've preserved the lifetime param, though we don't do anything with it.
    assert!(rec.generics.params[0].as_lifetime_param().is_some());

    let mut ty_param_iter = rec.generics.type_params();

    let first = ty_param_iter
        .next()
        .expect("type_params should not be empty");
    assert!(first.bar.is_none());
    assert!(first.foo);
    assert_eq!(first.ident, "T");

    let second = ty_param_iter
        .next()
        .expect("type_params should have a second value");
    assert_eq!(
        second
            .bar
            .as_ref()
            .expect("Second type param should set bar"),
        "x"
    );
    assert_eq!(second.foo, false);
    assert_eq!(second.ident, "U");
}

/// Verify ≤0.4.1 behavior - where `generics` had to be `syn::Generics` - keeps working.
#[test]
fn passthrough() {
    #[derive(FromDeriveInput)]
    struct PassthroughReceiver {
        pub generics: syn::Generics,
    }

    let rec: PassthroughReceiver = fdi(r#"
        struct Baz<
            'a,
            #[lorem(foo)] T,
            #[lorem(bar = "x")] U: Eq + ?Sized
        >(&'a T, U);
    "#)
    .expect("Input is well-formed");

    let mut type_param_iter = rec.generics.type_params();
    assert!(type_param_iter.next().is_some());
}

/// Verify that `where_clause` is passed through when it exists.
/// As of 0.4.1, there is no `FromWhereClause` trait, so other types aren't supported
/// for that field.
#[test]
fn where_clause() {
    let rec: MyReceiver = fdi(r#"
        struct Baz<
            'a,
            #[lorem(foo)] T,
            #[lorem(bar = "x")] U: Eq + ?Sized
        >(&'a T, U) where T: Into<String>;
    "#)
    .expect("Input is well-formed");

    assert!(rec.generics.where_clause.is_some());
}

/// Test that `WithOriginal` works for generics.
#[test]
fn with_original() {
    #[derive(FromDeriveInput)]
    struct WorigReceiver {
        generics: WithOriginal<ast::Generics<ast::GenericParam<MyTypeParam>>, syn::Generics>,
    }

    let rec: WorigReceiver = fdi(r#"
        struct Baz<
            'a,
            #[lorem(foo)] T,
            #[lorem(bar = "x")] U: Eq + ?Sized
        >(&'a T, U) where T: Into<String>;
    "#)
    .expect("Input is well-formed");

    // Make sure we haven't lost anything in the conversion
    assert_eq!(rec.generics.parsed.params.len(), 3);
    assert_eq!(rec.generics.original.params.len(), 3);

    let parsed_t: &MyTypeParam = rec.generics.parsed.params[1]
        .as_type_param()
        .expect("Second argument should be type param");

    // Make sure the first type param in each case is T
    assert_eq!(parsed_t.ident, "T");
    assert_eq!(
        rec.generics
            .original
            .type_params()
            .next()
            .expect("First type param should exist")
            .ident,
        "T"
    );

    // Make sure we actually parsed the first type param
    assert!(parsed_t.foo);
    assert!(parsed_t.bar.is_none());
}

/// Make sure generics can be ignored
#[test]
fn ignored() {
    #[derive(FromDeriveInput)]
    struct IgnoredReceiver {
        generics: Ignored,
    }

    let rec: IgnoredReceiver = fdi(r#"
        struct Baz<
            'a,
            #[lorem(foo)] T,
            #[lorem(bar = "x")] U: Eq + ?Sized
        >(&'a T, U) where T: Into<String>;
    "#)
    .expect("Input is well-formed");

    assert_eq!(Ignored, rec.generics);
}
