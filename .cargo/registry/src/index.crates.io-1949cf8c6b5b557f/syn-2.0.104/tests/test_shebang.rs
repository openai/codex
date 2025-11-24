#![allow(
    clippy::elidable_lifetime_names,
    clippy::needless_lifetimes,
    clippy::uninlined_format_args
)]

#[macro_use]
mod macros;

#[test]
fn test_basic() {
    let content = "#!/usr/bin/env rustx\nfn main() {}";
    let file = syn::parse_file(content).unwrap();
    snapshot!(file, @r##"
    File {
        shebang: Some("#!/usr/bin/env rustx"),
        items: [
            Item::Fn {
                vis: Visibility::Inherited,
                sig: Signature {
                    ident: "main",
                    generics: Generics,
                    output: ReturnType::Default,
                },
                block: Block {
                    stmts: [],
                },
            },
        ],
    }
    "##);
}

#[test]
fn test_comment() {
    let content = "#!//am/i/a/comment\n[allow(dead_code)] fn main() {}";
    let file = syn::parse_file(content).unwrap();
    snapshot!(file, @r#"
    File {
        attrs: [
            Attribute {
                style: AttrStyle::Inner,
                meta: Meta::List {
                    path: Path {
                        segments: [
                            PathSegment {
                                ident: "allow",
                            },
                        ],
                    },
                    delimiter: MacroDelimiter::Paren,
                    tokens: TokenStream(`dead_code`),
                },
            },
        ],
        items: [
            Item::Fn {
                vis: Visibility::Inherited,
                sig: Signature {
                    ident: "main",
                    generics: Generics,
                    output: ReturnType::Default,
                },
                block: Block {
                    stmts: [],
                },
            },
        ],
    }
    "#);
}
