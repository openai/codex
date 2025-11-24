#![allow(
    clippy::assertions_on_result_states,
    clippy::elidable_lifetime_names,
    clippy::manual_let_else,
    clippy::needless_lifetimes,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]

#[macro_use]
mod macros;

use quote::quote;
use syn::{Data, DeriveInput};

#[test]
fn test_unit() {
    let input = quote! {
        struct Unit;
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        vis: Visibility::Inherited,
        ident: "Unit",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Unit,
            semi_token: Some,
        },
    }
    "#);
}

#[test]
fn test_struct() {
    let input = quote! {
        #[derive(Debug, Clone)]
        pub struct Item {
            pub ident: Ident,
            pub attrs: Vec<Attribute>
        }
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        attrs: [
            Attribute {
                style: AttrStyle::Outer,
                meta: Meta::List {
                    path: Path {
                        segments: [
                            PathSegment {
                                ident: "derive",
                            },
                        ],
                    },
                    delimiter: MacroDelimiter::Paren,
                    tokens: TokenStream(`Debug , Clone`),
                },
            },
        ],
        vis: Visibility::Public,
        ident: "Item",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Named {
                named: [
                    Field {
                        vis: Visibility::Public,
                        ident: Some("ident"),
                        colon_token: Some,
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "Ident",
                                    },
                                ],
                            },
                        },
                    },
                    Token![,],
                    Field {
                        vis: Visibility::Public,
                        ident: Some("attrs"),
                        colon_token: Some,
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "Vec",
                                        arguments: PathArguments::AngleBracketed {
                                            args: [
                                                GenericArgument::Type(Type::Path {
                                                    path: Path {
                                                        segments: [
                                                            PathSegment {
                                                                ident: "Attribute",
                                                            },
                                                        ],
                                                    },
                                                }),
                                            ],
                                        },
                                    },
                                ],
                            },
                        },
                    },
                ],
            },
        },
    }
    "#);

    snapshot!(&input.attrs[0].meta, @r#"
    Meta::List {
        path: Path {
            segments: [
                PathSegment {
                    ident: "derive",
                },
            ],
        },
        delimiter: MacroDelimiter::Paren,
        tokens: TokenStream(`Debug , Clone`),
    }
    "#);
}

#[test]
fn test_union() {
    let input = quote! {
        union MaybeUninit<T> {
            uninit: (),
            value: T
        }
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        vis: Visibility::Inherited,
        ident: "MaybeUninit",
        generics: Generics {
            lt_token: Some,
            params: [
                GenericParam::Type(TypeParam {
                    ident: "T",
                }),
            ],
            gt_token: Some,
        },
        data: Data::Union {
            fields: FieldsNamed {
                named: [
                    Field {
                        vis: Visibility::Inherited,
                        ident: Some("uninit"),
                        colon_token: Some,
                        ty: Type::Tuple,
                    },
                    Token![,],
                    Field {
                        vis: Visibility::Inherited,
                        ident: Some("value"),
                        colon_token: Some,
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "T",
                                    },
                                ],
                            },
                        },
                    },
                ],
            },
        },
    }
    "#);
}

#[test]
#[cfg(feature = "full")]
fn test_enum() {
    let input = quote! {
        /// See the std::result module documentation for details.
        #[must_use]
        pub enum Result<T, E> {
            Ok(T),
            Err(E),
            Surprise = 0isize,

            // Smuggling data into a proc_macro_derive,
            // in the style of https://github.com/dtolnay/proc-macro-hack
            ProcMacroHack = (0, "data").0
        }
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        attrs: [
            Attribute {
                style: AttrStyle::Outer,
                meta: Meta::NameValue {
                    path: Path {
                        segments: [
                            PathSegment {
                                ident: "doc",
                            },
                        ],
                    },
                    value: Expr::Lit {
                        lit: " See the std::result module documentation for details.",
                    },
                },
            },
            Attribute {
                style: AttrStyle::Outer,
                meta: Meta::Path {
                    segments: [
                        PathSegment {
                            ident: "must_use",
                        },
                    ],
                },
            },
        ],
        vis: Visibility::Public,
        ident: "Result",
        generics: Generics {
            lt_token: Some,
            params: [
                GenericParam::Type(TypeParam {
                    ident: "T",
                }),
                Token![,],
                GenericParam::Type(TypeParam {
                    ident: "E",
                }),
            ],
            gt_token: Some,
        },
        data: Data::Enum {
            variants: [
                Variant {
                    ident: "Ok",
                    fields: Fields::Unnamed {
                        unnamed: [
                            Field {
                                vis: Visibility::Inherited,
                                ty: Type::Path {
                                    path: Path {
                                        segments: [
                                            PathSegment {
                                                ident: "T",
                                            },
                                        ],
                                    },
                                },
                            },
                        ],
                    },
                },
                Token![,],
                Variant {
                    ident: "Err",
                    fields: Fields::Unnamed {
                        unnamed: [
                            Field {
                                vis: Visibility::Inherited,
                                ty: Type::Path {
                                    path: Path {
                                        segments: [
                                            PathSegment {
                                                ident: "E",
                                            },
                                        ],
                                    },
                                },
                            },
                        ],
                    },
                },
                Token![,],
                Variant {
                    ident: "Surprise",
                    fields: Fields::Unit,
                    discriminant: Some(Expr::Lit {
                        lit: 0isize,
                    }),
                },
                Token![,],
                Variant {
                    ident: "ProcMacroHack",
                    fields: Fields::Unit,
                    discriminant: Some(Expr::Field {
                        base: Expr::Tuple {
                            elems: [
                                Expr::Lit {
                                    lit: 0,
                                },
                                Token![,],
                                Expr::Lit {
                                    lit: "data",
                                },
                            ],
                        },
                        member: Member::Unnamed(Index {
                            index: 0,
                        }),
                    }),
                },
            ],
        },
    }
    "#);

    let meta_items: Vec<_> = input.attrs.into_iter().map(|attr| attr.meta).collect();

    snapshot!(meta_items, @r#"
    [
        Meta::NameValue {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "doc",
                    },
                ],
            },
            value: Expr::Lit {
                lit: " See the std::result module documentation for details.",
            },
        },
        Meta::Path {
            segments: [
                PathSegment {
                    ident: "must_use",
                },
            ],
        },
    ]
    "#);
}

#[test]
fn test_attr_with_non_mod_style_path() {
    let input = quote! {
        #[inert <T>]
        struct S;
    };

    syn::parse2::<DeriveInput>(input).unwrap_err();
}

#[test]
fn test_attr_with_mod_style_path_with_self() {
    let input = quote! {
        #[foo::self]
        struct S;
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        attrs: [
            Attribute {
                style: AttrStyle::Outer,
                meta: Meta::Path {
                    segments: [
                        PathSegment {
                            ident: "foo",
                        },
                        Token![::],
                        PathSegment {
                            ident: "self",
                        },
                    ],
                },
            },
        ],
        vis: Visibility::Inherited,
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Unit,
            semi_token: Some,
        },
    }
    "#);

    snapshot!(&input.attrs[0].meta, @r#"
    Meta::Path {
        segments: [
            PathSegment {
                ident: "foo",
            },
            Token![::],
            PathSegment {
                ident: "self",
            },
        ],
    }
    "#);
}

#[test]
fn test_pub_restricted() {
    // Taken from tests/rust/src/test/ui/resolve/auxiliary/privacy-struct-ctor.rs
    let input = quote! {
        pub(in m) struct Z(pub(in m::n) u8);
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        vis: Visibility::Restricted {
            in_token: Some,
            path: Path {
                segments: [
                    PathSegment {
                        ident: "m",
                    },
                ],
            },
        },
        ident: "Z",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Unnamed {
                unnamed: [
                    Field {
                        vis: Visibility::Restricted {
                            in_token: Some,
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "m",
                                    },
                                    Token![::],
                                    PathSegment {
                                        ident: "n",
                                    },
                                ],
                            },
                        },
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "u8",
                                    },
                                ],
                            },
                        },
                    },
                ],
            },
            semi_token: Some,
        },
    }
    "#);
}

#[test]
fn test_pub_restricted_crate() {
    let input = quote! {
        pub(crate) struct S;
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        vis: Visibility::Restricted {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "crate",
                    },
                ],
            },
        },
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Unit,
            semi_token: Some,
        },
    }
    "#);
}

#[test]
fn test_pub_restricted_super() {
    let input = quote! {
        pub(super) struct S;
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        vis: Visibility::Restricted {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "super",
                    },
                ],
            },
        },
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Unit,
            semi_token: Some,
        },
    }
    "#);
}

#[test]
fn test_pub_restricted_in_super() {
    let input = quote! {
        pub(in super) struct S;
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        vis: Visibility::Restricted {
            in_token: Some,
            path: Path {
                segments: [
                    PathSegment {
                        ident: "super",
                    },
                ],
            },
        },
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Unit,
            semi_token: Some,
        },
    }
    "#);
}

#[test]
fn test_fields_on_unit_struct() {
    let input = quote! {
        struct S;
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        vis: Visibility::Inherited,
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Unit,
            semi_token: Some,
        },
    }
    "#);

    let data = match input.data {
        Data::Struct(data) => data,
        _ => panic!("expected a struct"),
    };

    assert_eq!(0, data.fields.iter().count());
}

#[test]
fn test_fields_on_named_struct() {
    let input = quote! {
        struct S {
            foo: i32,
            pub bar: String,
        }
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        vis: Visibility::Inherited,
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Named {
                named: [
                    Field {
                        vis: Visibility::Inherited,
                        ident: Some("foo"),
                        colon_token: Some,
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "i32",
                                    },
                                ],
                            },
                        },
                    },
                    Token![,],
                    Field {
                        vis: Visibility::Public,
                        ident: Some("bar"),
                        colon_token: Some,
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "String",
                                    },
                                ],
                            },
                        },
                    },
                    Token![,],
                ],
            },
        },
    }
    "#);

    let data = match input.data {
        Data::Struct(data) => data,
        _ => panic!("expected a struct"),
    };

    snapshot!(data.fields.into_iter().collect::<Vec<_>>(), @r#"
    [
        Field {
            vis: Visibility::Inherited,
            ident: Some("foo"),
            colon_token: Some,
            ty: Type::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "i32",
                        },
                    ],
                },
            },
        },
        Field {
            vis: Visibility::Public,
            ident: Some("bar"),
            colon_token: Some,
            ty: Type::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "String",
                        },
                    ],
                },
            },
        },
    ]
    "#);
}

#[test]
fn test_fields_on_tuple_struct() {
    let input = quote! {
        struct S(i32, pub String);
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        vis: Visibility::Inherited,
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Unnamed {
                unnamed: [
                    Field {
                        vis: Visibility::Inherited,
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "i32",
                                    },
                                ],
                            },
                        },
                    },
                    Token![,],
                    Field {
                        vis: Visibility::Public,
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "String",
                                    },
                                ],
                            },
                        },
                    },
                ],
            },
            semi_token: Some,
        },
    }
    "#);

    let data = match input.data {
        Data::Struct(data) => data,
        _ => panic!("expected a struct"),
    };

    snapshot!(data.fields.iter().collect::<Vec<_>>(), @r#"
    [
        Field {
            vis: Visibility::Inherited,
            ty: Type::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "i32",
                        },
                    ],
                },
            },
        },
        Field {
            vis: Visibility::Public,
            ty: Type::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "String",
                        },
                    ],
                },
            },
        },
    ]
    "#);
}

#[test]
fn test_ambiguous_crate() {
    let input = quote! {
        // The field type is `(crate::X)` not `crate (::X)`.
        struct S(crate::X);
    };

    snapshot!(input as DeriveInput, @r#"
    DeriveInput {
        vis: Visibility::Inherited,
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Unnamed {
                unnamed: [
                    Field {
                        vis: Visibility::Inherited,
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "crate",
                                    },
                                    Token![::],
                                    PathSegment {
                                        ident: "X",
                                    },
                                ],
                            },
                        },
                    },
                ],
            },
            semi_token: Some,
        },
    }
    "#);
}
