#[macro_use]
mod macros;

use proc_macro2::{Delimiter, Group, Ident, Span, TokenStream, TokenTree};
use quote::quote;
use std::iter::FromIterator;
use syn::{Item, ItemTrait};

#[test]
fn test_macro_variable_attr() {
    // mimics the token stream corresponding to `$attr fn f() {}`
    let tokens = TokenStream::from_iter(vec![
        TokenTree::Group(Group::new(Delimiter::None, quote! { #[test] })),
        TokenTree::Ident(Ident::new("fn", Span::call_site())),
        TokenTree::Ident(Ident::new("f", Span::call_site())),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Group(Group::new(Delimiter::Brace, TokenStream::new())),
    ]);

    snapshot!(tokens as Item, @r###"
    Item::Fn {
        attrs: [
            Attribute {
                style: Outer,
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "test",
                            arguments: None,
                        },
                    ],
                },
                tokens: TokenStream(``),
            },
        ],
        vis: Inherited,
        sig: Signature {
            ident: "f",
            generics: Generics,
            output: Default,
        },
        block: Block,
    }
    "###);
}

#[test]
fn test_negative_impl() {
    // Rustc parses all of the following.

    #[cfg(any())]
    impl ! {}
    let tokens = quote! {
        impl ! {}
    };
    snapshot!(tokens as Item, @r###"
    Item::Impl {
        generics: Generics,
        self_ty: Type::Never,
    }
    "###);

    #[cfg(any())]
    #[rustfmt::skip]
    impl !Trait {}
    let tokens = quote! {
        impl !Trait {}
    };
    snapshot!(tokens as Item, @r###"
    Item::Impl {
        generics: Generics,
        self_ty: Verbatim(`! Trait`),
    }
    "###);

    #[cfg(any())]
    impl !Trait for T {}
    let tokens = quote! {
        impl !Trait for T {}
    };
    snapshot!(tokens as Item, @r###"
    Item::Impl {
        generics: Generics,
        trait_: Some((
            Some,
            Path {
                segments: [
                    PathSegment {
                        ident: "Trait",
                        arguments: None,
                    },
                ],
            },
        )),
        self_ty: Type::Path {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "T",
                        arguments: None,
                    },
                ],
            },
        },
    }
    "###);

    #[cfg(any())]
    #[rustfmt::skip]
    impl !! {}
    let tokens = quote! {
        impl !! {}
    };
    snapshot!(tokens as Item, @r###"
    Item::Impl {
        generics: Generics,
        self_ty: Verbatim(`! !`),
    }
    "###);
}

#[test]
fn test_macro_variable_impl() {
    // mimics the token stream corresponding to `impl $trait for $ty {}`
    let tokens = TokenStream::from_iter(vec![
        TokenTree::Ident(Ident::new("impl", Span::call_site())),
        TokenTree::Group(Group::new(Delimiter::None, quote!(Trait))),
        TokenTree::Ident(Ident::new("for", Span::call_site())),
        TokenTree::Group(Group::new(Delimiter::None, quote!(Type))),
        TokenTree::Group(Group::new(Delimiter::Brace, TokenStream::new())),
    ]);

    snapshot!(tokens as Item, @r###"
    Item::Impl {
        generics: Generics,
        trait_: Some((
            None,
            Path {
                segments: [
                    PathSegment {
                        ident: "Trait",
                        arguments: None,
                    },
                ],
            },
        )),
        self_ty: Type::Group {
            elem: Type::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "Type",
                            arguments: None,
                        },
                    ],
                },
            },
        },
    }
    "###);
}

#[test]
fn test_supertraits() {
    // Rustc parses all of the following.

    #[rustfmt::skip]
    let tokens = quote!(trait Trait where {});
    snapshot!(tokens as ItemTrait, @r###"
    ItemTrait {
        vis: Inherited,
        ident: "Trait",
        generics: Generics {
            where_clause: Some(WhereClause),
        },
    }
    "###);

    #[rustfmt::skip]
    let tokens = quote!(trait Trait: where {});
    snapshot!(tokens as ItemTrait, @r###"
    ItemTrait {
        vis: Inherited,
        ident: "Trait",
        generics: Generics {
            where_clause: Some(WhereClause),
        },
        colon_token: Some,
    }
    "###);

    #[rustfmt::skip]
    let tokens = quote!(trait Trait: Sized where {});
    snapshot!(tokens as ItemTrait, @r###"
    ItemTrait {
        vis: Inherited,
        ident: "Trait",
        generics: Generics {
            where_clause: Some(WhereClause),
        },
        colon_token: Some,
        supertraits: [
            Trait(TraitBound {
                modifier: None,
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "Sized",
                            arguments: None,
                        },
                    ],
                },
            }),
        ],
    }
    "###);

    #[rustfmt::skip]
    let tokens = quote!(trait Trait: Sized + where {});
    snapshot!(tokens as ItemTrait, @r###"
    ItemTrait {
        vis: Inherited,
        ident: "Trait",
        generics: Generics {
            where_clause: Some(WhereClause),
        },
        colon_token: Some,
        supertraits: [
            Trait(TraitBound {
                modifier: None,
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "Sized",
                            arguments: None,
                        },
                    ],
                },
            }),
        ],
    }
    "###);
}

#[test]
fn test_type_empty_bounds() {
    #[rustfmt::skip]
    let tokens = quote! {
        trait Foo {
            type Bar: ;
        }
    };

    snapshot!(tokens as ItemTrait, @r###"
    ItemTrait {
        vis: Inherited,
        ident: "Foo",
        generics: Generics,
        items: [
            TraitItem::Type {
                ident: "Bar",
                generics: Generics,
                colon_token: Some,
            },
        ],
    }
    "###);
}

#[test]
fn test_impl_visibility() {
    let tokens = quote! {
        pub default unsafe impl union {}
    };

    snapshot!(tokens as Item, @"Verbatim(`pub default unsafe impl union { }`)");
}

#[test]
fn test_impl_type_parameter_defaults() {
    #[cfg(any())]
    impl<T = ()> () {}
    let tokens = quote! {
        impl<T = ()> () {}
    };
    snapshot!(tokens as Item, @r###"
    Item::Impl {
        generics: Generics {
            lt_token: Some,
            params: [
                Type(TypeParam {
                    ident: "T",
                    eq_token: Some,
                    default: Some(Type::Tuple),
                }),
            ],
            gt_token: Some,
        },
        self_ty: Type::Tuple,
    }"###);
}

#[test]
fn test_impl_trait_trailing_plus() {
    let tokens = quote! {
        fn f() -> impl Sized + {}
    };

    snapshot!(tokens as Item, @r###"
    Item::Fn {
        vis: Inherited,
        sig: Signature {
            ident: "f",
            generics: Generics,
            output: Type(
                Type::ImplTrait {
                    bounds: [
                        Trait(TraitBound {
                            modifier: None,
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "Sized",
                                        arguments: None,
                                    },
                                ],
                            },
                        }),
                    ],
                },
            ),
        },
        block: Block,
    }
    "###);
}
