#[macro_use]
mod macros;

use syn::parse::Parser;
use syn::{Attribute, Meta};

#[test]
fn test_meta_item_word() {
    let meta = test("#[foo]");

    snapshot!(meta, @r###"
    Path(Path {
        segments: [
            PathSegment {
                ident: "foo",
                arguments: None,
            },
        ],
    })
    "###);
}

#[test]
fn test_meta_item_name_value() {
    let meta = test("#[foo = 5]");

    snapshot!(meta, @r###"
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
}

#[test]
fn test_meta_item_bool_value() {
    let meta = test("#[foo = true]");

    snapshot!(meta, @r###"
    Meta::NameValue {
        path: Path {
            segments: [
                PathSegment {
                    ident: "foo",
                    arguments: None,
                },
            ],
        },
        lit: Lit::Bool {
            value: true,
        },
    }
    "###);

    let meta = test("#[foo = false]");

    snapshot!(meta, @r###"
    Meta::NameValue {
        path: Path {
            segments: [
                PathSegment {
                    ident: "foo",
                    arguments: None,
                },
            ],
        },
        lit: Lit::Bool {
            value: false,
        },
    }
    "###);
}

#[test]
fn test_meta_item_list_lit() {
    let meta = test("#[foo(5)]");

    snapshot!(meta, @r###"
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
}

#[test]
fn test_meta_item_list_word() {
    let meta = test("#[foo(bar)]");

    snapshot!(meta, @r###"
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
                        ident: "bar",
                        arguments: None,
                    },
                ],
            })),
        ],
    }
    "###);
}

#[test]
fn test_meta_item_list_name_value() {
    let meta = test("#[foo(bar = 5)]");

    snapshot!(meta, @r###"
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
            Meta(Meta::NameValue {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "bar",
                            arguments: None,
                        },
                    ],
                },
                lit: 5,
            }),
        ],
    }
    "###);
}

#[test]
fn test_meta_item_list_bool_value() {
    let meta = test("#[foo(bar = true)]");

    snapshot!(meta, @r###"
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
            Meta(Meta::NameValue {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "bar",
                            arguments: None,
                        },
                    ],
                },
                lit: Lit::Bool {
                    value: true,
                },
            }),
        ],
    }
    "###);
}

#[test]
fn test_meta_item_multiple() {
    let meta = test("#[foo(word, name = 5, list(name2 = 6), word2)]");

    snapshot!(meta, @r###"
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
}

#[test]
fn test_bool_lit() {
    let meta = test("#[foo(true)]");

    snapshot!(meta, @r###"
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
            Lit(Lit::Bool {
                value: true,
            }),
        ],
    }
    "###);
}

#[test]
fn test_negative_lit() {
    let meta = test("#[form(min = -1, max = 200)]");

    snapshot!(meta, @r###"
    Meta::List {
        path: Path {
            segments: [
                PathSegment {
                    ident: "form",
                    arguments: None,
                },
            ],
        },
        nested: [
            Meta(Meta::NameValue {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "min",
                            arguments: None,
                        },
                    ],
                },
                lit: -1,
            }),
            Meta(Meta::NameValue {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "max",
                            arguments: None,
                        },
                    ],
                },
                lit: 200,
            }),
        ],
    }
    "###);
}

fn test(input: &str) -> Meta {
    let attrs = Attribute::parse_outer.parse_str(input).unwrap();

    assert_eq!(attrs.len(), 1);
    let attr = attrs.into_iter().next().unwrap();

    attr.parse_meta().unwrap()
}
