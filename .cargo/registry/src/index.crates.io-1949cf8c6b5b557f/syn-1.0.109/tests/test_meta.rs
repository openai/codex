#![allow(clippy::shadow_unrelated, clippy::too_many_lines)]

#[macro_use]
mod macros;

use syn::{Meta, MetaList, MetaNameValue, NestedMeta};

#[test]
fn test_parse_meta_item_word() {
    let input = "hello";

    snapshot!(input as Meta, @r###"
    Path(Path {
        segments: [
            PathSegment {
                ident: "hello",
                arguments: None,
            },
        ],
    })
    "###);
}

#[test]
fn test_parse_meta_name_value() {
    let input = "foo = 5";
    let (inner, meta) = (input, input);

    snapshot!(inner as MetaNameValue, @r###"
    MetaNameValue {
        path: Path {
            segments: [
                PathSegment {
                    ident: "foo",
                    arguments: None,
                },
            ],
        },
        lit: 5,
    }
    "###);

    snapshot!(meta as Meta, @r###"
    Meta::NameValue {
        path: Path {
            segments: [
                PathSegment {
                    ident: "foo",
                    arguments: None,
                },
            ],
        },
        lit: 5,
    }
    "###);

    assert_eq!(meta, inner.into());
}

#[test]
fn test_parse_meta_name_value_with_keyword() {
    let input = "static = 5";
    let (inner, meta) = (input, input);

    snapshot!(inner as MetaNameValue, @r###"
    MetaNameValue {
        path: Path {
            segments: [
                PathSegment {
                    ident: "static",
                    arguments: None,
                },
            ],
        },
        lit: 5,
    }
    "###);

    snapshot!(meta as Meta, @r###"
    Meta::NameValue {
        path: Path {
            segments: [
                PathSegment {
                    ident: "static",
                    arguments: None,
                },
            ],
        },
        lit: 5,
    }
    "###);

    assert_eq!(meta, inner.into());
}

#[test]
fn test_parse_meta_name_value_with_bool() {
    let input = "true = 5";
    let (inner, meta) = (input, input);

    snapshot!(inner as MetaNameValue, @r###"
    MetaNameValue {
        path: Path {
            segments: [
                PathSegment {
                    ident: "true",
                    arguments: None,
                },
            ],
        },
        lit: 5,
    }
    "###);

    snapshot!(meta as Meta, @r###"
    Meta::NameValue {
        path: Path {
            segments: [
                PathSegment {
                    ident: "true",
                    arguments: None,
                },
            ],
        },
        lit: 5,
    }
    "###);

    assert_eq!(meta, inner.into());
}

#[test]
fn test_parse_meta_item_list_lit() {
    let input = "foo(5)";
    let (inner, meta) = (input, input);

    snapshot!(inner as MetaList, @r###"
    MetaList {
        path: Path {
            segments: [
                PathSegment {
                    ident: "foo",
                    arguments: None,
                },
            ],
        },
        nested: [
            Lit(5),
        ],
    }
    "###);

    snapshot!(meta as Meta, @r###"
    Meta::List {
        path: Path {
            segments: [
                PathSegment {
                    ident: "foo",
                    arguments: None,
                },
            ],
        },
        nested: [
            Lit(5),
        ],
    }
    "###);

    assert_eq!(meta, inner.into());
}

#[test]
fn test_parse_meta_item_multiple() {
    let input = "foo(word, name = 5, list(name2 = 6), word2)";
    let (inner, meta) = (input, input);

    snapshot!(inner as MetaList, @r###"
    MetaList {
        path: Path {
            segments: [
                PathSegment {
                    ident: "foo",
                    arguments: None,
                },
            ],
        },
        nested: [
            Meta(Path(Path {
                segments: [
                    PathSegment {
                        ident: "word",
                        arguments: None,
                    },
                ],
            })),
            Meta(Meta::NameValue {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "name",
                            arguments: None,
                        },
                    ],
                },
                lit: 5,
            }),
            Meta(Meta::List {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "list",
                            arguments: None,
                        },
                    ],
                },
                nested: [
                    Meta(Meta::NameValue {
                        path: Path {
                            segments: [
                                PathSegment {
                                    ident: "name2",
                                    arguments: None,
                                },
                            ],
                        },
                        lit: 6,
                    }),
                ],
            }),
            Meta(Path(Path {
                segments: [
                    PathSegment {
                        ident: "word2",
                        arguments: None,
                    },
                ],
            })),
        ],
    }
    "###);

    snapshot!(meta as Meta, @r###"
    Meta::List {
        path: Path {
            segments: [
                PathSegment {
                    ident: "foo",
                    arguments: None,
                },
            ],
        },
        nested: [
            Meta(Path(Path {
                segments: [
                    PathSegment {
                        ident: "word",
                        arguments: None,
                    },
                ],
            })),
            Meta(Meta::NameValue {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "name",
                            arguments: None,
                        },
                    ],
                },
                lit: 5,
            }),
            Meta(Meta::List {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "list",
                            arguments: None,
                        },
                    ],
                },
                nested: [
                    Meta(Meta::NameValue {
                        path: Path {
                            segments: [
                                PathSegment {
                                    ident: "name2",
                                    arguments: None,
                                },
                            ],
                        },
                        lit: 6,
                    }),
                ],
            }),
            Meta(Path(Path {
                segments: [
                    PathSegment {
                        ident: "word2",
                        arguments: None,
                    },
                ],
            })),
        ],
    }
    "###);

    assert_eq!(meta, inner.into());
}

#[test]
fn test_parse_nested_meta() {
    let input = "5";
    snapshot!(input as NestedMeta, @"Lit(5)");

    let input = "list(name2 = 6)";
    snapshot!(input as NestedMeta, @r###"
    Meta(Meta::List {
        path: Path {
            segments: [
                PathSegment {
                    ident: "list",
                    arguments: None,
                },
            ],
        },
        nested: [
            Meta(Meta::NameValue {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "name2",
                            arguments: None,
                        },
                    ],
                },
                lit: 6,
            }),
        ],
    })
    "###);
}

#[test]
fn test_parse_path() {
    let input = "::serde::Serialize";
    snapshot!(input as Meta, @r###"
    Path(Path {
        leading_colon: Some,
        segments: [
            PathSegment {
                ident: "serde",
                arguments: None,
            },
            PathSegment {
                ident: "Serialize",
                arguments: None,
            },
        ],
    })
    "###);

    let input = "::serde::Serialize";
    snapshot!(input as NestedMeta, @r###"
    Meta(Path(Path {
        leading_colon: Some,
        segments: [
            PathSegment {
                ident: "serde",
                arguments: None,
            },
            PathSegment {
                ident: "Serialize",
                arguments: None,
            },
        ],
    }))
    "###);
}
