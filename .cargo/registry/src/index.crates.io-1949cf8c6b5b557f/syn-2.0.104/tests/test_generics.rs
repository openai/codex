#![allow(
    clippy::elidable_lifetime_names,
    clippy::manual_let_else,
    clippy::needless_lifetimes,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]

#[macro_use]
mod macros;

use quote::quote;
use syn::{DeriveInput, ItemFn, TypeParamBound, WhereClause, WherePredicate};

#[test]
fn test_split_for_impl() {
    let input = quote! {
        struct S<'a, 'b: 'a, #[may_dangle] T: 'a = ()> where T: Debug;
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        vis: Visibility::Inherited,
        ident: "S",
        generics: Generics {
            lt_token: Some,
            params: [
                GenericParam::Lifetime(LifetimeParam {
                    lifetime: Lifetime {
                        ident: "a",
                    },
                }),
                Token![,],
                GenericParam::Lifetime(LifetimeParam {
                    lifetime: Lifetime {
                        ident: "b",
                    },
                    colon_token: Some,
                    bounds: [
                        Lifetime {
                            ident: "a",
                        },
                    ],
                }),
                Token![,],
                GenericParam::Type(TypeParam {
                    attrs: [
                        Attribute {
                            style: AttrStyle::Outer,
                            meta: Meta::Path {
                                segments: [
                                    PathSegment {
                                        ident: "may_dangle",
                                    },
                                ],
                            },
                        },
                    ],
                    ident: "T",
                    colon_token: Some,
                    bounds: [
                        TypeParamBound::Lifetime {
                            ident: "a",
                        },
                    ],
                    eq_token: Some,
                    default: Some(Type::Tuple),
                }),
            ],
            gt_token: Some,
            where_clause: Some(WhereClause {
                predicates: [
                    WherePredicate::Type(PredicateType {
                        bounded_ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "T",
                                    },
                                ],
                            },
                        },
                        bounds: [
                            TypeParamBound::Trait(TraitBound {
                                path: Path {
                                    segments: [
                                        PathSegment {
                                            ident: "Debug",
                                        },
                                    ],
                                },
                            }),
                        ],
                    }),
                ],
            }),
        },
        data: Data::Struct {
            fields: Fields::Unit,
            semi_token: Some,
        },
    }
    "#);

    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let generated = quote! {
        impl #impl_generics MyTrait for Test #ty_generics #where_clause {}
    };
    let expected = quote! {
        impl<'a, 'b: 'a, #[may_dangle] T: 'a> MyTrait
        for Test<'a, 'b, T>
        where
            T: Debug
        {}
    };
    assert_eq!(generated.to_string(), expected.to_string());

    let turbofish = ty_generics.as_turbofish();
    let generated = quote! {
        Test #turbofish
    };
    let expected = quote! {
        Test::<'a, 'b, T>
    };
    assert_eq!(generated.to_string(), expected.to_string());
}

#[test]
fn test_ty_param_bound() {
    let tokens = quote!('a);
    snapshot!(tokens as TypeParamBound, @r#"
    TypeParamBound::Lifetime {
        ident: "a",
    }
    "#);

    let tokens = quote!('_);
    snapshot!(tokens as TypeParamBound, @r#"
    TypeParamBound::Lifetime {
        ident: "_",
    }
    "#);

    let tokens = quote!(Debug);
    snapshot!(tokens as TypeParamBound, @r#"
    TypeParamBound::Trait(TraitBound {
        path: Path {
            segments: [
                PathSegment {
                    ident: "Debug",
                },
            ],
        },
    })
    "#);

    let tokens = quote!(?Sized);
    snapshot!(tokens as TypeParamBound, @r#"
    TypeParamBound::Trait(TraitBound {
        modifier: TraitBoundModifier::Maybe,
        path: Path {
            segments: [
                PathSegment {
                    ident: "Sized",
                },
            ],
        },
    })
    "#);
}

#[test]
fn test_fn_precedence_in_where_clause() {
    // This should parse as two separate bounds, `FnOnce() -> i32` and `Send` - not
    // `FnOnce() -> (i32 + Send)`.
    let input = quote! {
        fn f<G>()
        where
            G: FnOnce() -> i32 + Send,
        {
        }
    };

    snapshot!(input as ItemFn, @r#"
    ItemFn {
        vis: Visibility::Inherited,
        sig: Signature {
            ident: "f",
            generics: Generics {
                lt_token: Some,
                params: [
                    GenericParam::Type(TypeParam {
                        ident: "G",
                    }),
                ],
                gt_token: Some,
                where_clause: Some(WhereClause {
                    predicates: [
                        WherePredicate::Type(PredicateType {
                            bounded_ty: Type::Path {
                                path: Path {
                                    segments: [
                                        PathSegment {
                                            ident: "G",
                                        },
                                    ],
                                },
                            },
                            bounds: [
                                TypeParamBound::Trait(TraitBound {
                                    path: Path {
                                        segments: [
                                            PathSegment {
                                                ident: "FnOnce",
                                                arguments: PathArguments::Parenthesized {
                                                    output: ReturnType::Type(
                                                        Type::Path {
                                                            path: Path {
                                                                segments: [
                                                                    PathSegment {
                                                                        ident: "i32",
                                                                    },
                                                                ],
                                                            },
                                                        },
                                                    ),
                                                },
                                            },
                                        ],
                                    },
                                }),
                                Token![+],
                                TypeParamBound::Trait(TraitBound {
                                    path: Path {
                                        segments: [
                                            PathSegment {
                                                ident: "Send",
                                            },
                                        ],
                                    },
                                }),
                            ],
                        }),
                        Token![,],
                    ],
                }),
            },
            output: ReturnType::Default,
        },
        block: Block {
            stmts: [],
        },
    }
    "#);

    let where_clause = input.sig.generics.where_clause.as_ref().unwrap();
    assert_eq!(where_clause.predicates.len(), 1);

    let predicate = match &where_clause.predicates[0] {
        WherePredicate::Type(pred) => pred,
        _ => panic!("wrong predicate kind"),
    };

    assert_eq!(predicate.bounds.len(), 2, "{:#?}", predicate.bounds);

    let first_bound = &predicate.bounds[0];
    assert_eq!(quote!(#first_bound).to_string(), "FnOnce () -> i32");

    let second_bound = &predicate.bounds[1];
    assert_eq!(quote!(#second_bound).to_string(), "Send");
}

#[test]
fn test_where_clause_at_end_of_input() {
    let input = quote! {
        where
    };

    snapshot!(input as WhereClause, @"WhereClause");

    assert_eq!(input.predicates.len(), 0);
}
