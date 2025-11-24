#![allow(
    clippy::elidable_lifetime_names,
    clippy::needless_lifetimes,
    clippy::uninlined_format_args
)]

#[macro_use]
mod macros;

use proc_macro2::{Delimiter, Group, Ident, Span, TokenStream, TokenTree};
use quote::quote;
use syn::{Item, ItemTrait};

#[test]
fn test_macro_variable_attr() {
    // mimics the token stream corresponding to `$attr fn f() {}`
    let tokens = TokenStream::from_iter([
        TokenTree::Group(Group::new(Delimiter::None, quote! { #[test] })),
        TokenTree::Ident(Ident::new("fn", Span::call_site())),
        TokenTree::Ident(Ident::new("f", Span::call_site())),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Group(Group::new(Delimiter::Brace, TokenStream::new())),
    ]);

    snapshot!(tokens as Item, @r#"
    Item::Fn {
        attrs: [
            Attribute {
                style: AttrStyle::Outer,
                meta: Meta::Path {
                    segments: [
                        PathSegment {
                            ident: "test",
                        },
                    ],
                },
            },
        ],
        vis: Visibility::Inherited,
        sig: Signature {
            ident: "f",
            generics: Generics,
            output: ReturnType::Default,
        },
        block: Block {
            stmts: [],
        },
    }
    "#);
}

#[test]
fn test_negative_impl() {
    // Rustc parses all of the following.

    #[cfg(any())]
    impl ! {}
    let tokens = quote! {
        impl ! {}
    };
    snapshot!(tokens as Item, @r#"
    Item::Impl {
        generics: Generics,
        self_ty: Type::Never,
    }
    "#);

    #[cfg(any())]
    #[rustfmt::skip]
    impl !Trait {}
    let tokens = quote! {
        impl !Trait {}
    };
    snapshot!(tokens as Item, @r#"
    Item::Impl {
        generics: Generics,
        self_ty: Type::Verbatim(`! Trait`),
    }
    "#);

    #[cfg(any())]
    impl !Trait for T {}
    let tokens = quote! {
        impl !Trait for T {}
    };
    snapshot!(tokens as Item, @r#"
    Item::Impl {
        generics: Generics,
        trait_: Some((
            Some,
            Path {
                segments: [
                    PathSegment {
                        ident: "Trait",
                    },
                ],
            },
        )),
        self_ty: Type::Path {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "T",
                    },
                ],
            },
        },
    }
    "#);

    #[cfg(any())]
    #[rustfmt::skip]
    impl !! {}
    let tokens = quote! {
        impl !! {}
    };
    snapshot!(tokens as Item, @r#"
    Item::Impl {
        generics: Generics,
        self_ty: Type::Verbatim(`! !`),
    }
    "#);
}

#[test]
fn test_macro_variable_impl() {
    // mimics the token stream corresponding to `impl $trait for $ty {}`
    let tokens = TokenStream::from_iter([
        TokenTree::Ident(Ident::new("impl", Span::call_site())),
        TokenTree::Group(Group::new(Delimiter::None, quote!(Trait))),
        TokenTree::Ident(Ident::new("for", Span::call_site())),
        TokenTree::Group(Group::new(Delimiter::None, quote!(Type))),
        TokenTree::Group(Group::new(Delimiter::Brace, TokenStream::new())),
    ]);

    snapshot!(tokens as Item, @r#"
    Item::Impl {
        generics: Generics,
        trait_: Some((
            None,
            Path {
                segments: [
                    PathSegment {
                        ident: "Trait",
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
                        },
                    ],
                },
            },
        },
    }
    "#);
}

#[test]
fn test_supertraits() {
    // Rustc parses all of the following.

    #[rustfmt::skip]
    let tokens = quote!(trait Trait where {});
    snapshot!(tokens as ItemTrait, @r#"
    ItemTrait {
        vis: Visibility::Inherited,
        ident: "Trait",
        generics: Generics {
            where_clause: Some(WhereClause),
        },
    }
    "#);

    #[rustfmt::skip]
    let tokens = quote!(trait Trait: where {});
    snapshot!(tokens as ItemTrait, @r#"
    ItemTrait {
        vis: Visibility::Inherited,
        ident: "Trait",
        generics: Generics {
            where_clause: Some(WhereClause),
        },
        colon_token: Some,
    }
    "#);

    #[rustfmt::skip]
    let tokens = quote!(trait Trait: Sized where {});
    snapshot!(tokens as ItemTrait, @r#"
    ItemTrait {
        vis: Visibility::Inherited,
        ident: "Trait",
        generics: Generics {
            where_clause: Some(WhereClause),
        },
        colon_token: Some,
        supertraits: [
            TypeParamBound::Trait(TraitBound {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "Sized",
                        },
                    ],
                },
            }),
        ],
    }
    "#);

    #[rustfmt::skip]
    let tokens = quote!(trait Trait: Sized + where {});
    snapshot!(tokens as ItemTrait, @r#"
    ItemTrait {
        vis: Visibility::Inherited,
        ident: "Trait",
        generics: Generics {
            where_clause: Some(WhereClause),
        },
        colon_token: Some,
        supertraits: [
            TypeParamBound::Trait(TraitBound {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "Sized",
                        },
                    ],
                },
            }),
            Token![+],
        ],
    }
    "#);
}

#[test]
fn test_type_empty_bounds() {
    #[rustfmt::skip]
    let tokens = quote! {
        trait Foo {
            type Bar: ;
        }
    };

    snapshot!(tokens as ItemTrait, @r#"
    ItemTrait {
        vis: Visibility::Inherited,
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
    "#);
}

#[test]
fn test_impl_visibility() {
    let tokens = quote! {
        pub default unsafe impl union {}
    };

    snapshot!(tokens as Item, @"Item::Verbatim(`pub default unsafe impl union { }`)");
}

#[test]
fn test_impl_type_parameter_defaults() {
    #[cfg(any())]
    impl<T = ()> () {}
    let tokens = quote! {
        impl<T = ()> () {}
    };
    snapshot!(tokens as Item, @r#"
    Item::Impl {
        generics: Generics {
            lt_token: Some,
            params: [
                GenericParam::Type(TypeParam {
                    ident: "T",
                    eq_token: Some,
                    default: Some(Type::Tuple),
                }),
            ],
            gt_token: Some,
        },
        self_ty: Type::Tuple,
    }
    "#);
}

#[test]
fn test_impl_trait_trailing_plus() {
    let tokens = quote! {
        fn f() -> impl Sized + {}
    };

    snapshot!(tokens as Item, @r#"
    Item::Fn {
        vis: Visibility::Inherited,
        sig: Signature {
            ident: "f",
            generics: Generics,
            output: ReturnType::Type(
                Type::ImplTrait {
                    bounds: [
                        TypeParamBound::Trait(TraitBound {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "Sized",
                                    },
                                ],
                            },
                        }),
                        Token![+],
                    ],
                },
            ),
        },
        block: Block {
            stmts: [],
        },
    }
    "#);
}
