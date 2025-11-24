//! This is a regression test for issue [#371](https://github.com/TedDriggs/darling/issues/371).

use darling::util::{Callable, Override};
use darling::FromDeriveInput;

#[derive(FromDeriveInput)]
#[darling(attributes(test))]
struct Test1 {
    #[allow(dead_code)]
    func: Callable,
}

#[derive(FromDeriveInput)]
#[darling(attributes(test))]
struct Test2 {
    func: Override<Callable>,
}

#[test]
fn test_explicit_closure() {
    let input = syn::parse_quote! {
        #[test(func = || 1 + 1)]
        struct Foo;
    };
    assert!(Test1::from_derive_input(&input).is_ok());
    assert!(Test2::from_derive_input(&input).is_ok());
}

#[test]
fn test_explicit_path() {
    let input = syn::parse_quote! {
        #[test(func = foo::bar)]
        struct Foo;
    };
    assert!(Test1::from_derive_input(&input).is_ok());
    assert!(Test2::from_derive_input(&input).is_ok());
}

#[test]
fn test_inherit() {
    let input = syn::parse_quote! {
        #[test(func)]
        struct Foo;
    };
    assert!(Test1::from_derive_input(&input).is_err());
    assert!(matches!(
        Test2::from_derive_input(&input).unwrap().func,
        Override::Inherit
    ));
}
