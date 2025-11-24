#[macro_use]
mod macros;

#[test]
fn test_basic() {
    let content = "#!/usr/bin/env rustx\nfn main() {}";
    let file = syn::parse_file(content).unwrap();
    snapshot!(file, @r###"
    File {
        shebang: Some("#!/usr/bin/env rustx"),
        items: [
            Item::Fn {
                vis: Inherited,
                sig: Signature {
                    ident: "main",
                    generics: Generics,
                    output: Default,
                },
                block: Block,
            },
        ],
    }
    "###);
}

#[test]
fn test_comment() {
    let content = "#!//am/i/a/comment\n[allow(dead_code)] fn main() {}";
    let file = syn::parse_file(content).unwrap();
    snapshot!(file, @r###"
    File {
        attrs: [
            Attribute {
                style: Inner,
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "allow",
                            arguments: None,
                        },
                    ],
                },
                tokens: TokenStream(`(dead_code)`),
            },
        ],
        items: [
            Item::Fn {
                vis: Inherited,
                sig: Signature {
                    ident: "main",
                    generics: Generics,
                    output: Default,
                },
                block: Block,
            },
        ],
    }
    "###);
}
