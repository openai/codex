#![allow(clippy::assertions_on_result_states, clippy::too_many_lines)]

#[macro_use]
mod macros;

use quote::quote;
use syn::{Data, DeriveInput};

#[test]
fn test_unit() {
    let input = quote! {
        struct Unit;
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        vis: Inherited,
        ident: "Unit",
        generics: Generics,
        data: Data::Struct {
            fields: Unit,
            semi_token: Some,
        },
    }
    "###);
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

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        attrs: [
            Attribute {
                style: Outer,
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "derive",
                            arguments: None,
                        },
                    ],
                },
                tokens: TokenStream(`(Debug , Clone)`),
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
                                        arguments: None,
                                    },
                                ],
                            },
                        },
                    },
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
                                                Type(Type::Path {
                                                    path: Path {
                                                        segments: [
                                                            PathSegment {
                                                                ident: "Attribute",
                                                                arguments: None,
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
    "###);

    snapshot!(input.attrs[0].parse_meta().unwrap(), @r###"
    Meta::List {
        path: Path {
            segments: [
                PathSegment {
                    ident: "derive",
                    arguments: None,
                },
            ],
        },
        nested: [
            Meta(Path(Path {
                segments: [
                    PathSegment {
                        ident: "Debug",
                        arguments: None,
                    },
                ],
            })),
            Meta(Path(Path {
                segments: [
                    PathSegment {
                        ident: "Clone",
                        arguments: None,
                    },
                ],
            })),
        ],
    }
    "###);
}

#[test]
fn test_union() {
    let input = quote! {
        union MaybeUninit<T> {
            uninit: (),
            value: T
        }
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        vis: Inherited,
        ident: "MaybeUninit",
        generics: Generics {
            lt_token: Some,
            params: [
                Type(TypeParam {
                    ident: "T",
                }),
            ],
            gt_token: Some,
        },
        data: Data::Union {
            fields: FieldsNamed {
                named: [
                    Field {
                        vis: Inherited,
                        ident: Some("uninit"),
                        colon_token: Some,
                        ty: Type::Tuple,
                    },
                    Field {
                        vis: Inherited,
                        ident: Some("value"),
                        colon_token: Some,
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "T",
                                        arguments: None,
                                    },
                                ],
                            },
                        },
                    },
                ],
            },
        },
    }
    "###);
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

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        attrs: [
            Attribute {
                style: Outer,
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "doc",
                            arguments: None,
                        },
                    ],
                },
                tokens: TokenStream(`= r" See the std::result module documentation for details."`),
            },
            Attribute {
                style: Outer,
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "must_use",
                            arguments: None,
                        },
                    ],
                },
                tokens: TokenStream(``),
            },
        ],
        vis: Visibility::Public,
        ident: "Result",
        generics: Generics {
            lt_token: Some,
            params: [
                Type(TypeParam {
                    ident: "T",
                }),
                Type(TypeParam {
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
                                vis: Inherited,
                                ty: Type::Path {
                                    path: Path {
                                        segments: [
                                            PathSegment {
                                                ident: "T",
                                                arguments: None,
                                            },
                                        ],
                                    },
                                },
                            },
                        ],
                    },
                },
                Variant {
                    ident: "Err",
                    fields: Fields::Unnamed {
                        unnamed: [
                            Field {
                                vis: Inherited,
                                ty: Type::Path {
                                    path: Path {
                                        segments: [
                                            PathSegment {
                                                ident: "E",
                                                arguments: None,
                                            },
                                        ],
                                    },
                                },
                            },
                        ],
                    },
                },
                Variant {
                    ident: "Surprise",
                    fields: Unit,
                    discriminant: Some(Expr::Lit {
                        lit: 0isize,
                    }),
                },
                Variant {
                    ident: "ProcMacroHack",
                    fields: Unit,
                    discriminant: Some(Expr::Field {
                        base: Expr::Tuple {
                            elems: [
                                Expr::Lit {
                                    lit: 0,
                                },
                                Expr::Lit {
                                    lit: "data",
                                },
                            ],
                        },
                        member: Unnamed(Index {
                            index: 0,
                        }),
                    }),
                },
            ],
        },
    }
    "###);

    let meta_items: Vec<_> = input
        .attrs
        .into_iter()
        .map(|attr| attr.parse_meta().unwrap())
        .collect();

    snapshot!(meta_items, @r###"
    [
        Meta::NameValue {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "doc",
                        arguments: None,
                    },
                ],
            },
            lit: " See the std::result module documentation for details.",
        },
        Path(Path {
            segments: [
                PathSegment {
                    ident: "must_use",
                    arguments: None,
                },
            ],
        }),
    ]
    "###);
}

#[test]
fn test_attr_with_path() {
    let input = quote! {
        #[::attr_args::identity
            fn main() { assert_eq!(foo(), "Hello, world!"); }]
        struct Dummy;
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        attrs: [
            Attribute {
                style: Outer,
                path: Path {
                    leading_colon: Some,
                    segments: [
                        PathSegment {
                            ident: "attr_args",
                            arguments: None,
                        },
                        PathSegment {
                            ident: "identity",
                            arguments: None,
                        },
                    ],
                },
                tokens: TokenStream(`fn main () { assert_eq ! (foo () , "Hello, world!") ; }`),
            },
        ],
        vis: Inherited,
        ident: "Dummy",
        generics: Generics,
        data: Data::Struct {
            fields: Unit,
            semi_token: Some,
        },
    }
    "###);

    assert!(input.attrs[0].parse_meta().is_err());
}

#[test]
fn test_attr_with_non_mod_style_path() {
    let input = quote! {
        #[inert <T>]
        struct S;
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        attrs: [
            Attribute {
                style: Outer,
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "inert",
                            arguments: None,
                        },
                    ],
                },
                tokens: TokenStream(`< T >`),
            },
        ],
        vis: Inherited,
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Unit,
            semi_token: Some,
        },
    }
    "###);

    assert!(input.attrs[0].parse_meta().is_err());
}

#[test]
fn test_attr_with_mod_style_path_with_self() {
    let input = quote! {
        #[foo::self]
        struct S;
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        attrs: [
            Attribute {
                style: Outer,
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "foo",
                            arguments: None,
                        },
                        PathSegment {
                            ident: "self",
                            arguments: None,
                        },
                    ],
                },
                tokens: TokenStream(``),
            },
        ],
        vis: Inherited,
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Unit,
            semi_token: Some,
        },
    }
    "###);

    snapshot!(input.attrs[0].parse_meta().unwrap(), @r###"
    Path(Path {
        segments: [
            PathSegment {
                ident: "foo",
                arguments: None,
            },
            PathSegment {
                ident: "self",
                arguments: None,
            },
        ],
    })
    "###);
}

#[test]
fn test_pub_restricted() {
    // Taken from tests/rust/src/test/ui/resolve/auxiliary/privacy-struct-ctor.rs
    let input = quote! {
        pub(in m) struct Z(pub(in m::n) u8);
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        vis: Visibility::Restricted {
            in_token: Some,
            path: Path {
                segments: [
                    PathSegment {
                        ident: "m",
                        arguments: None,
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
                                        arguments: None,
                                    },
                                    PathSegment {
                                        ident: "n",
                                        arguments: None,
                                    },
                                ],
                            },
                        },
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "u8",
                                        arguments: None,
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
    "###);
}

#[test]
fn test_vis_crate() {
    let input = quote! {
        crate struct S;
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        vis: Visibility::Crate,
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Unit,
            semi_token: Some,
        },
    }
    "###);
}

#[test]
fn test_pub_restricted_crate() {
    let input = quote! {
        pub(crate) struct S;
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        vis: Visibility::Restricted {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "crate",
                        arguments: None,
                    },
                ],
            },
        },
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Unit,
            semi_token: Some,
        },
    }
    "###);
}

#[test]
fn test_pub_restricted_super() {
    let input = quote! {
        pub(super) struct S;
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        vis: Visibility::Restricted {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "super",
                        arguments: None,
                    },
                ],
            },
        },
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Unit,
            semi_token: Some,
        },
    }
    "###);
}

#[test]
fn test_pub_restricted_in_super() {
    let input = quote! {
        pub(in super) struct S;
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        vis: Visibility::Restricted {
            in_token: Some,
            path: Path {
                segments: [
                    PathSegment {
                        ident: "super",
                        arguments: None,
                    },
                ],
            },
        },
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Unit,
            semi_token: Some,
        },
    }
    "###);
}

#[test]
fn test_fields_on_unit_struct() {
    let input = quote! {
        struct S;
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        vis: Inherited,
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Unit,
            semi_token: Some,
        },
    }
    "###);

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

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        vis: Inherited,
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Named {
                named: [
                    Field {
                        vis: Inherited,
                        ident: Some("foo"),
                        colon_token: Some,
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "i32",
                                        arguments: None,
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
                                        arguments: None,
                                    },
                                ],
                            },
                        },
                    },
                ],
            },
        },
    }
    "###);

    let data = match input.data {
        Data::Struct(data) => data,
        _ => panic!("expected a struct"),
    };

    snapshot!(data.fields.into_iter().collect::<Vec<_>>(), @r###"
    [
        Field {
            vis: Inherited,
            ident: Some("foo"),
            colon_token: Some,
            ty: Type::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "i32",
                            arguments: None,
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
                            arguments: None,
                        },
                    ],
                },
            },
        },
    ]
    "###);
}

#[test]
fn test_fields_on_tuple_struct() {
    let input = quote! {
        struct S(i32, pub String);
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        vis: Inherited,
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Unnamed {
                unnamed: [
                    Field {
                        vis: Inherited,
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "i32",
                                        arguments: None,
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
                                        arguments: None,
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
    "###);

    let data = match input.data {
        Data::Struct(data) => data,
        _ => panic!("expected a struct"),
    };

    snapshot!(data.fields.iter().collect::<Vec<_>>(), @r###"
    [
        Field {
            vis: Inherited,
            ty: Type::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "i32",
                            arguments: None,
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
                            arguments: None,
                        },
                    ],
                },
            },
        },
    ]
    "###);
}

#[test]
fn test_ambiguous_crate() {
    let input = quote! {
        // The field type is `(crate::X)` not `crate (::X)`.
        struct S(crate::X);
    };

    snapshot!(input as DeriveInput, @r###"
    DeriveInput {
        vis: Inherited,
        ident: "S",
        generics: Generics,
        data: Data::Struct {
            fields: Fields::Unnamed {
                unnamed: [
                    Field {
                        vis: Inherited,
                        ty: Type::Path {
                            path: Path {
                                segments: [
                                    PathSegment {
                                        ident: "crate",
                                        arguments: None,
                                    },
                                    PathSegment {
                                        ident: "X",
                                        arguments: None,
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
    "###);
}
