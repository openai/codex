use crate::node::{Lit, Whitespace, Ws};
use crate::{
    Ast, Expr, Filter, InnerSyntax, Node, Num, Span, StrLit, Syntax, SyntaxBuilder, WithSpan,
};

impl<T> WithSpan<'static, T> {
    fn no_span(inner: T) -> Self {
        Self {
            inner,
            span: Span::default(),
        }
    }
}

fn check_ws_split(s: &str, res: &(&str, &str, &str)) {
    let Lit { lws, val, rws } = Lit::split_ws_parts(s);
    assert_eq!(lws, res.0);
    assert_eq!(val, res.1);
    assert_eq!(rws, res.2);
}

#[test]
fn test_ws_splitter() {
    check_ws_split("", &("", "", ""));
    check_ws_split("a", &("", "a", ""));
    check_ws_split("\ta", &("\t", "a", ""));
    check_ws_split("b\n", &("", "b", "\n"));
    check_ws_split(" \t\r\n", &(" \t\r\n", "", ""));
}

#[test]
#[should_panic]
fn test_invalid_block() {
    Ast::from_str("{% extend \"blah\" %}", None, &Syntax::default()).unwrap();
}

fn int_lit(i: &str) -> Expr<'_> {
    Expr::NumLit(i, Num::Int(i, None))
}

#[test]
fn test_parse_filter() {
    let syntax = Syntax::default();
    assert_eq!(
        Ast::from_str("{{ strvar|e }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Filter(Filter {
                name: "e",
                arguments: vec![WithSpan::no_span(Expr::Var("strvar"))],
                generics: vec![],
            })),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ 2|abs }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Filter(Filter {
                name: "abs",
                arguments: vec![WithSpan::no_span(int_lit("2"))],
                generics: vec![],
            })),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ -2|abs }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Filter(Filter {
                name: "abs",
                arguments: vec![WithSpan::no_span(Expr::Unary(
                    "-",
                    WithSpan::no_span(int_lit("2")).into()
                ))],
                generics: vec![],
            })),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (1 - 2)|abs }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Filter(Filter {
                name: "abs",
                arguments: vec![WithSpan::no_span(Expr::Group(
                    WithSpan::no_span(Expr::BinOp(
                        "-",
                        WithSpan::no_span(int_lit("1")).into(),
                        WithSpan::no_span(int_lit("2")).into()
                    ))
                    .into()
                ))],
                generics: vec![],
            })),
        )],
    );
}

#[test]
fn test_parse_numbers() {
    let syntax = Syntax::default();
    assert_eq!(
        Ast::from_str("{{ 2 }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(Ws(None, None), WithSpan::no_span(int_lit("2")))],
    );
    assert_eq!(
        Ast::from_str("{{ 2.5 }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::NumLit("2.5", Num::Float("2.5", None)))
        )],
    );
}

#[test]
fn test_parse_var() {
    let s = Syntax::default();

    assert_eq!(
        Ast::from_str("{{ foo }}", None, &s).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Var("foo"))
        )]
    );
    assert_eq!(
        Ast::from_str("{{ foo_bar }}", None, &s).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Var("foo_bar"))
        )],
    );

    assert_eq!(
        Ast::from_str("{{ none }}", None, &s).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Var("none"))
        )]
    );
}

#[test]
fn test_parse_const() {
    let s = Syntax::default();

    assert_eq!(
        Ast::from_str("{{ FOO }}", None, &s).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Path(vec!["FOO"]))
        )]
    );
    assert_eq!(
        Ast::from_str("{{ FOO_BAR }}", None, &s).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Path(vec!["FOO_BAR"]))
        )],
    );

    assert_eq!(
        Ast::from_str("{{ NONE }}", None, &s).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Path(vec!["NONE"]))
        )]
    );
}

#[test]
fn test_parse_path() {
    let s = Syntax::default();

    assert_eq!(
        Ast::from_str("{{ None }}", None, &s).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Path(vec!["None"]))
        )]
    );
    assert_eq!(
        Ast::from_str("{{ Some(123) }}", None, &s).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Call {
                path: Box::new(WithSpan::no_span(Expr::Path(vec!["Some"]))),
                args: vec![WithSpan::no_span(int_lit("123"))],
                generics: vec![],
            }),
        )],
    );

    assert_eq!(
        Ast::from_str("{{ Ok(123) }}", None, &s).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Call {
                path: Box::new(WithSpan::no_span(Expr::Path(vec!["Ok"]))),
                args: vec![WithSpan::no_span(int_lit("123"))],
                generics: vec![],
            }),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ Err(123) }}", None, &s).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Call {
                path: Box::new(WithSpan::no_span(Expr::Path(vec!["Err"]))),
                args: vec![WithSpan::no_span(int_lit("123"))],
                generics: vec![],
            }),
        )],
    );
}

#[test]
fn test_parse_var_call() {
    assert_eq!(
        Ast::from_str("{{ function(\"123\", 3) }}", None, &Syntax::default())
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Call {
                path: Box::new(WithSpan::no_span(Expr::Var("function"))),
                args: vec![
                    WithSpan::no_span(Expr::StrLit(StrLit {
                        content: "123",
                        prefix: None,
                    })),
                    WithSpan::no_span(int_lit("3"))
                ],
                generics: vec![],
            }),
        )],
    );
}

#[test]
fn test_parse_path_call() {
    let s = Syntax::default();

    assert_eq!(
        Ast::from_str("{{ Option::None }}", None, &s).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Path(vec!["Option", "None"]))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ Option::Some(123) }}", None, &s)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Call {
                path: Box::new(WithSpan::no_span(Expr::Path(vec!["Option", "Some"]))),
                args: vec![WithSpan::no_span(int_lit("123"))],
                generics: vec![],
            })
        )],
    );

    assert_eq!(
        Ast::from_str("{{ self::function(\"123\", 3) }}", None, &s)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Call {
                path: Box::new(WithSpan::no_span(Expr::Path(vec!["self", "function"]))),
                args: vec![
                    WithSpan::no_span(Expr::StrLit(StrLit {
                        content: "123",
                        prefix: None,
                    })),
                    WithSpan::no_span(int_lit("3"))
                ],
                generics: vec![],
            })
        )],
    );
}

#[test]
fn test_parse_root_path() {
    let syntax = Syntax::default();
    assert_eq!(
        Ast::from_str("{{ std::string::String::new() }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Call {
                path: Box::new(WithSpan::no_span(Expr::Path(vec![
                    "std", "string", "String", "new"
                ]))),
                args: vec![],
                generics: vec![],
            }),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ ::std::string::String::new() }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Call {
                path: Box::new(WithSpan::no_span(Expr::Path(vec![
                    "", "std", "string", "String", "new"
                ]))),
                args: vec![],
                generics: vec![],
            }),
        )],
    );
}

#[test]
fn test_rust_macro() {
    let syntax = Syntax::default();
    assert_eq!(
        Ast::from_str("{{ vec!(1, 2, 3) }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::RustMacro(vec!["vec"], "1, 2, 3")),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ alloc::vec!(1, 2, 3) }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::RustMacro(vec!["alloc", "vec"], "1, 2, 3")),
        )],
    );
    assert_eq!(
        Ast::from_str("{{a!()}}", None, &syntax).unwrap().nodes,
        [Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::RustMacro(vec!["a"], ""))
        )]
    );
    assert_eq!(
        Ast::from_str("{{a !()}}", None, &syntax).unwrap().nodes,
        [Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::RustMacro(vec!["a"], ""))
        )]
    );
    assert_eq!(
        Ast::from_str("{{a! ()}}", None, &syntax).unwrap().nodes,
        [Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::RustMacro(vec!["a"], ""))
        )]
    );
    assert_eq!(
        Ast::from_str("{{a ! ()}}", None, &syntax).unwrap().nodes,
        [Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::RustMacro(vec!["a"], ""))
        )]
    );
    assert_eq!(
        Ast::from_str("{{A!()}}", None, &syntax).unwrap().nodes,
        [Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::RustMacro(vec!["A"], ""))
        )]
    );
    assert_eq!(
        &*Ast::from_str("{{a.b.c!( hello )}}", None, &syntax)
            .unwrap_err()
            .to_string(),
        "failed to parse template source near offset 7",
    );
}

#[test]
fn change_delimiters_parse_filter() {
    let syntax = Syntax(InnerSyntax {
        expr_start: "{=",
        expr_end: "=}",
        ..InnerSyntax::default()
    });
    Ast::from_str("{= strvar|e =}", None, &syntax).unwrap();
}

#[test]
fn unicode_delimiters_in_syntax() {
    let syntax = Syntax(InnerSyntax {
        expr_start: "🖎", // U+1F58E == b"\xf0\x9f\x96\x8e"
        expr_end: "✍",   // U+270D = b'\xe2\x9c\x8d'
        ..InnerSyntax::default()
    });
    assert_eq!(
        Ast::from_str("Here comes the expression: 🖎 e ✍.", None, &syntax)
            .unwrap()
            .nodes(),
        [
            Node::Lit(WithSpan::no_span(Lit {
                lws: "",
                val: "Here comes the expression:",
                rws: " ",
            })),
            Node::Expr(Ws(None, None), WithSpan::no_span(Expr::Var("e"))),
            Node::Lit(WithSpan::no_span(Lit {
                lws: "",
                val: ".",
                rws: "",
            })),
        ],
    );
}

#[test]
fn test_precedence() {
    let syntax = Syntax::default();
    assert_eq!(
        Ast::from_str("{{ a + b == c }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::BinOp(
                "==",
                WithSpan::no_span(Expr::BinOp(
                    "+",
                    WithSpan::no_span(Expr::Var("a")).into(),
                    WithSpan::no_span(Expr::Var("b")).into()
                ))
                .into(),
                WithSpan::no_span(Expr::Var("c")).into()
            ))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ a + b * c - d / e }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::BinOp(
                "-",
                WithSpan::no_span(Expr::BinOp(
                    "+",
                    WithSpan::no_span(Expr::Var("a")).into(),
                    WithSpan::no_span(Expr::BinOp(
                        "*",
                        WithSpan::no_span(Expr::Var("b")).into(),
                        WithSpan::no_span(Expr::Var("c")).into()
                    ))
                    .into()
                ))
                .into(),
                WithSpan::no_span(Expr::BinOp(
                    "/",
                    WithSpan::no_span(Expr::Var("d")).into(),
                    WithSpan::no_span(Expr::Var("e")).into()
                ))
                .into(),
            ))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ a * (b + c) / -d }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::BinOp(
                "/",
                Box::new(WithSpan::no_span(Expr::BinOp(
                    "*",
                    Box::new(WithSpan::no_span(Expr::Var("a"))),
                    Box::new(WithSpan::no_span(Expr::Group(Box::new(WithSpan::no_span(
                        Expr::BinOp(
                            "+",
                            Box::new(WithSpan::no_span(Expr::Var("b"))),
                            Box::new(WithSpan::no_span(Expr::Var("c")))
                        )
                    )))))
                ))),
                Box::new(WithSpan::no_span(Expr::Unary(
                    "-",
                    Box::new(WithSpan::no_span(Expr::Var("d")))
                )))
            ))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ a || b && c || d && e }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::BinOp(
                "||",
                Box::new(WithSpan::no_span(Expr::BinOp(
                    "||",
                    Box::new(WithSpan::no_span(Expr::Var("a"))),
                    Box::new(WithSpan::no_span(Expr::BinOp(
                        "&&",
                        Box::new(WithSpan::no_span(Expr::Var("b"))),
                        Box::new(WithSpan::no_span(Expr::Var("c")))
                    ))),
                ))),
                Box::new(WithSpan::no_span(Expr::BinOp(
                    "&&",
                    Box::new(WithSpan::no_span(Expr::Var("d"))),
                    Box::new(WithSpan::no_span(Expr::Var("e")))
                ))),
            ))
        )],
    );
}

#[test]
fn test_associativity() {
    let syntax = Syntax::default();
    assert_eq!(
        Ast::from_str("{{ a + b + c }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::BinOp(
                "+",
                Box::new(WithSpan::no_span(Expr::BinOp(
                    "+",
                    Box::new(WithSpan::no_span(Expr::Var("a"))),
                    Box::new(WithSpan::no_span(Expr::Var("b")))
                ))),
                Box::new(WithSpan::no_span(Expr::Var("c")))
            ))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ a * b * c }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::BinOp(
                "*",
                Box::new(WithSpan::no_span(Expr::BinOp(
                    "*",
                    Box::new(WithSpan::no_span(Expr::Var("a"))),
                    Box::new(WithSpan::no_span(Expr::Var("b")))
                ))),
                Box::new(WithSpan::no_span(Expr::Var("c")))
            ))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ a && b && c }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::BinOp(
                "&&",
                Box::new(WithSpan::no_span(Expr::BinOp(
                    "&&",
                    Box::new(WithSpan::no_span(Expr::Var("a"))),
                    Box::new(WithSpan::no_span(Expr::Var("b")))
                ))),
                Box::new(WithSpan::no_span(Expr::Var("c")))
            ))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ a + b - c + d }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::BinOp(
                "+",
                Box::new(WithSpan::no_span(Expr::BinOp(
                    "-",
                    Box::new(WithSpan::no_span(Expr::BinOp(
                        "+",
                        Box::new(WithSpan::no_span(Expr::Var("a"))),
                        Box::new(WithSpan::no_span(Expr::Var("b")))
                    ))),
                    Box::new(WithSpan::no_span(Expr::Var("c")))
                ))),
                Box::new(WithSpan::no_span(Expr::Var("d")))
            ))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ a == b != c > d > e == f }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::BinOp(
                "==",
                Box::new(WithSpan::no_span(Expr::BinOp(
                    ">",
                    Box::new(WithSpan::no_span(Expr::BinOp(
                        ">",
                        Box::new(WithSpan::no_span(Expr::BinOp(
                            "!=",
                            Box::new(WithSpan::no_span(Expr::BinOp(
                                "==",
                                Box::new(WithSpan::no_span(Expr::Var("a"))),
                                Box::new(WithSpan::no_span(Expr::Var("b")))
                            ))),
                            Box::new(WithSpan::no_span(Expr::Var("c")))
                        ))),
                        Box::new(WithSpan::no_span(Expr::Var("d")))
                    ))),
                    Box::new(WithSpan::no_span(Expr::Var("e")))
                ))),
                Box::new(WithSpan::no_span(Expr::Var("f")))
            ))
        )],
    );
}

#[test]
fn test_odd_calls() {
    let syntax = Syntax::default();
    assert_eq!(
        Ast::from_str("{{ a[b](c) }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Call {
                path: Box::new(WithSpan::no_span(Expr::Index(
                    Box::new(WithSpan::no_span(Expr::Var("a"))),
                    Box::new(WithSpan::no_span(Expr::Var("b")))
                ))),
                args: vec![WithSpan::no_span(Expr::Var("c"))],
                generics: vec![],
            })
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (a + b)(c) }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Call {
                path: Box::new(WithSpan::no_span(Expr::Group(Box::new(WithSpan::no_span(
                    Expr::BinOp(
                        "+",
                        Box::new(WithSpan::no_span(Expr::Var("a"))),
                        Box::new(WithSpan::no_span(Expr::Var("b")))
                    )
                ))))),
                args: vec![WithSpan::no_span(Expr::Var("c"))],
                generics: vec![],
            })
        )],
    );
    assert_eq!(
        Ast::from_str("{{ a + b(c) }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::BinOp(
                "+",
                Box::new(WithSpan::no_span(Expr::Var("a"))),
                Box::new(WithSpan::no_span(Expr::Call {
                    path: Box::new(WithSpan::no_span(Expr::Var("b"))),
                    args: vec![WithSpan::no_span(Expr::Var("c"))],
                    generics: vec![],
                })),
            )),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (-a)(b) }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Call {
                path: Box::new(WithSpan::no_span(Expr::Group(Box::new(WithSpan::no_span(
                    Expr::Unary("-", Box::new(WithSpan::no_span(Expr::Var("a"))))
                ))))),
                args: vec![WithSpan::no_span(Expr::Var("b"))],
                generics: vec![],
            })
        )],
    );
    assert_eq!(
        Ast::from_str("{{ -a(b) }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Unary(
                "-",
                Box::new(WithSpan::no_span(Expr::Call {
                    path: Box::new(WithSpan::no_span(Expr::Var("a"))),
                    args: vec![WithSpan::no_span(Expr::Var("b"))],
                    generics: vec![],
                }))
            ))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ a(b)|c }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Filter(Filter {
                name: "c",
                arguments: vec![WithSpan::no_span(Expr::Call {
                    path: Box::new(WithSpan::no_span(Expr::Var("a"))),
                    args: vec![WithSpan::no_span(Expr::Var("b"))],
                    generics: vec![],
                })],
                generics: vec![],
            }))
        )]
    );
    assert_eq!(
        Ast::from_str("{{ a(b)| c }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Filter(Filter {
                name: "c",
                arguments: vec![WithSpan::no_span(Expr::Call {
                    path: Box::new(WithSpan::no_span(Expr::Var("a"))),
                    args: vec![WithSpan::no_span(Expr::Var("b"))],
                    generics: vec![],
                })],
                generics: vec![],
            })),
        )]
    );
}

#[test]
fn test_parse_comments() {
    #[track_caller]
    fn one_comment_ws(source: &str, ws: Ws) {
        let s = &Syntax::default();
        let mut nodes = Ast::from_str(source, None, s).unwrap().nodes;
        assert_eq!(nodes.len(), 1, "expected to parse one node");
        match nodes.pop().unwrap() {
            Node::Comment(comment) => assert_eq!(comment.ws, ws),
            node => panic!("expected a comment not, but parsed {node:?}"),
        }
    }

    one_comment_ws("{##}", Ws(None, None));
    one_comment_ws("{#- #}", Ws(Some(Whitespace::Suppress), None));
    one_comment_ws("{# -#}", Ws(None, Some(Whitespace::Suppress)));
    one_comment_ws(
        "{#--#}",
        Ws(Some(Whitespace::Suppress), Some(Whitespace::Suppress)),
    );
    one_comment_ws(
        "{#- foo\n bar -#}",
        Ws(Some(Whitespace::Suppress), Some(Whitespace::Suppress)),
    );
    one_comment_ws(
        "{#- foo\n {#- bar\n -#} baz -#}",
        Ws(Some(Whitespace::Suppress), Some(Whitespace::Suppress)),
    );
    one_comment_ws("{#+ #}", Ws(Some(Whitespace::Preserve), None));
    one_comment_ws("{# +#}", Ws(None, Some(Whitespace::Preserve)));
    one_comment_ws(
        "{#++#}",
        Ws(Some(Whitespace::Preserve), Some(Whitespace::Preserve)),
    );
    one_comment_ws(
        "{#+ foo\n bar +#}",
        Ws(Some(Whitespace::Preserve), Some(Whitespace::Preserve)),
    );
    one_comment_ws(
        "{#+ foo\n {#+ bar\n +#} baz -+#}",
        Ws(Some(Whitespace::Preserve), Some(Whitespace::Preserve)),
    );
    one_comment_ws("{#~ #}", Ws(Some(Whitespace::Minimize), None));
    one_comment_ws("{# ~#}", Ws(None, Some(Whitespace::Minimize)));
    one_comment_ws(
        "{#~~#}",
        Ws(Some(Whitespace::Minimize), Some(Whitespace::Minimize)),
    );
    one_comment_ws(
        "{#~ foo\n bar ~#}",
        Ws(Some(Whitespace::Minimize), Some(Whitespace::Minimize)),
    );
    one_comment_ws(
        "{#~ foo\n {#~ bar\n ~#} baz -~#}",
        Ws(Some(Whitespace::Minimize), Some(Whitespace::Minimize)),
    );

    one_comment_ws("{# foo {# bar #} {# {# baz #} qux #} #}", Ws(None, None));
}

#[test]
fn test_parse_tuple() {
    let syntax = Syntax::default();
    assert_eq!(
        Ast::from_str("{{ () }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Tuple(vec![]))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (1) }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Group(Box::new(WithSpan::no_span(int_lit("1")))))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (1,) }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Tuple(vec![WithSpan::no_span(int_lit("1"))])),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (1, ) }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Tuple(vec![WithSpan::no_span(int_lit("1"))])),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (1 ,) }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Tuple(vec![WithSpan::no_span(int_lit("1"))])),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (1 , ) }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Tuple(vec![WithSpan::no_span(int_lit("1"))])),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (1, 2) }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Tuple(vec![
                WithSpan::no_span(int_lit("1")),
                WithSpan::no_span(int_lit("2"))
            ])),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (1, 2,) }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Tuple(vec![
                WithSpan::no_span(int_lit("1")),
                WithSpan::no_span(int_lit("2"))
            ])),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (1, 2, 3) }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Tuple(vec![
                WithSpan::no_span(int_lit("1")),
                WithSpan::no_span(int_lit("2")),
                WithSpan::no_span(int_lit("3"))
            ])),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ ()|abs }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Filter(Filter {
                name: "abs",
                arguments: vec![WithSpan::no_span(Expr::Tuple(vec![]))],
                generics: vec![],
            })),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (1)|abs }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Filter(Filter {
                name: "abs",
                arguments: vec![WithSpan::no_span(Expr::Group(Box::new(WithSpan::no_span(
                    int_lit("1")
                ))))],
                generics: vec![],
            })),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (1,)|abs }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Filter(Filter {
                name: "abs",
                arguments: vec![WithSpan::no_span(Expr::Tuple(vec![WithSpan::no_span(
                    int_lit("1")
                )]))],
                generics: vec![],
            })),
        )],
    );
    assert_eq!(
        Ast::from_str("{{ (1, 2)|abs }}", None, &syntax)
            .unwrap()
            .nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Filter(Filter {
                name: "abs",
                arguments: vec![WithSpan::no_span(Expr::Tuple(vec![
                    WithSpan::no_span(int_lit("1")),
                    WithSpan::no_span(int_lit("2"))
                ]))],
                generics: vec![],
            })),
        )],
    );
}

#[test]
fn test_missing_space_after_kw() {
    let syntax = Syntax::default();
    let err = Ast::from_str("{%leta=b%}", None, &syntax).unwrap_err();
    assert_eq!(
        err.to_string(),
        "unknown node `leta`\nfailed to parse template source near offset 2",
    );
}

#[test]
fn test_parse_array() {
    let syntax = Syntax::default();
    assert_eq!(
        Ast::from_str("{{ [] }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Array(vec![]))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ [1] }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Array(vec![WithSpan::no_span(int_lit("1"))]))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ [ 1] }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Array(vec![WithSpan::no_span(int_lit("1"))]))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ [1 ] }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Array(vec![WithSpan::no_span(int_lit("1"))]))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ [1,2] }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Array(vec![
                WithSpan::no_span(int_lit("1")),
                WithSpan::no_span(int_lit("2"))
            ]))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ [1 ,2] }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Array(vec![
                WithSpan::no_span(int_lit("1")),
                WithSpan::no_span(int_lit("2"))
            ]))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ [1, 2] }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Array(vec![
                WithSpan::no_span(int_lit("1")),
                WithSpan::no_span(int_lit("2"))
            ]))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ [1,2 ] }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Array(vec![
                WithSpan::no_span(int_lit("1")),
                WithSpan::no_span(int_lit("2"))
            ]))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ []|foo }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Filter(Filter {
                name: "foo",
                arguments: vec![WithSpan::no_span(Expr::Array(vec![]))],
                generics: vec![],
            }))
        )],
    );
    assert_eq!(
        Ast::from_str("{{ []| foo }}", None, &syntax).unwrap().nodes,
        vec![Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Filter(Filter {
                name: "foo",
                arguments: vec![WithSpan::no_span(Expr::Array(vec![]))],
                generics: vec![],
            }))
        )],
    );

    let n = || {
        Node::Expr(
            Ws(None, None),
            WithSpan::no_span(Expr::Array(vec![WithSpan::no_span(Expr::NumLit(
                "1",
                Num::Int("1", None),
            ))])),
        )
    };
    assert_eq!(
        Ast::from_str(
            "{{ [1,] }}{{ [1 ,] }}{{ [1, ] }}{{ [1 , ] }}",
            None,
            &syntax
        )
        .unwrap()
        .nodes,
        vec![n(), n(), n(), n()],
    );
}

#[test]
fn fuzzed_unicode_slice() {
    let d = "{eeuuu{b&{!!&{!!11{{
            0!(!1q҄א!)!!!!!!n!";
    assert!(Ast::from_str(d, None, &Syntax::default()).is_err());
}

#[test]
fn fuzzed_macro_no_end() {
    let s = "{%macro super%}{%endmacro";
    assert!(Ast::from_str(s, None, &Syntax::default()).is_err());
}

#[test]
fn fuzzed_target_recursion() {
    const TEMPLATE: &str = include_str!("../tests/target-recursion.txt");
    assert!(Ast::from_str(TEMPLATE, None, &Syntax::default()).is_err());
}

#[test]
fn fuzzed_unary_recursion() {
    const TEMPLATE: &str = include_str!("../tests/unary-recursion.txt");
    assert!(Ast::from_str(TEMPLATE, None, &Syntax::default()).is_err());
}

#[test]
fn fuzzed_comment_depth() {
    let (sender, receiver) = std::sync::mpsc::channel();
    let test = std::thread::spawn(move || {
        const TEMPLATE: &str = include_str!("../tests/comment-depth.txt");
        assert!(Ast::from_str(TEMPLATE, None, &Syntax::default()).is_ok());
        sender.send(()).unwrap();
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(3))
        .expect("timeout");
    test.join().unwrap();
}

#[test]
fn let_set() {
    assert_eq!(
        Ast::from_str("{% let a %}", None, &Syntax::default())
            .unwrap()
            .nodes(),
        Ast::from_str("{% set a %}", None, &Syntax::default())
            .unwrap()
            .nodes(),
    );
}

#[test]
fn fuzzed_filter_recursion() {
    const TEMPLATE: &str = include_str!("../tests/filter-recursion.txt");
    assert!(Ast::from_str(TEMPLATE, None, &Syntax::default()).is_err());
}

#[test]
fn fuzzed_excessive_syntax_lengths() {
    const LONG_DELIM: Option<&str> =
        Some("\0]***NEWFILE\u{1f}***:7/v/.-3/\u{1b}/~~~~z~0/*:7/v/./t/t/.p//NEWVILE**::7/v");

    for (kind, syntax_builder) in [
        (
            "opening block",
            SyntaxBuilder {
                block_start: LONG_DELIM,
                ..SyntaxBuilder::default()
            },
        ),
        (
            "closing block",
            SyntaxBuilder {
                block_end: LONG_DELIM,
                ..SyntaxBuilder::default()
            },
        ),
        (
            "opening expression",
            SyntaxBuilder {
                expr_start: LONG_DELIM,
                ..SyntaxBuilder::default()
            },
        ),
        (
            "closing expression",
            SyntaxBuilder {
                expr_end: LONG_DELIM,
                ..SyntaxBuilder::default()
            },
        ),
        (
            "opening comment",
            SyntaxBuilder {
                comment_start: LONG_DELIM,
                ..SyntaxBuilder::default()
            },
        ),
        (
            "closing comment",
            SyntaxBuilder {
                comment_end: LONG_DELIM,
                ..SyntaxBuilder::default()
            },
        ),
    ] {
        let err = syntax_builder.to_syntax().unwrap_err();
        assert_eq!(
            err,
            format!(
                "delimiters must be at most 32 characters long. The {} delimiter \
                 (\"\\0]***NEWFILE\\u{{1f}}***\"...) is too long",
                kind
            ),
        );
    }
}

#[test]
fn extends_with_whitespace_control() {
    const CONTROL: &[&str] = &["", "\t", "-", "+", "~"];

    let syntax = Syntax::default();
    let expected = Ast::from_str(r#"front {% extends "nothing" %} back"#, None, &syntax).unwrap();
    for front in CONTROL {
        for back in CONTROL {
            let src = format!(r#"front {{%{front} extends "nothing" {back}%}} back"#);
            let actual = Ast::from_str(&src, None, &syntax).unwrap();
            assert_eq!(expected.nodes(), actual.nodes(), "source: {:?}", src);
        }
    }
}

#[test]
fn fuzzed_span_is_not_substring_of_source() {
    let _: Result<Ast<'_>, crate::ParseError> = Ast::from_str(
        include_str!("../tests/fuzzed_span_is_not_substring_of_source.bin"),
        None,
        &Syntax::default(),
    );
}

#[test]
fn fuzzed_excessive_filter_block() {
    let src = include_str!("../tests/excessive_filter_block.txt");
    let err = Ast::from_str(src, None, &Syntax::default()).unwrap_err();
    assert_eq!(
        err.to_string().lines().next(),
        Some("your template code is too deeply nested, or the last expression is too complex"),
    );

    let src = include!("../tests/fuzzed_excessive_filter_block.inc");
    let err = Ast::from_str(src, None, &Syntax::default()).unwrap_err();
    assert_eq!(
        err.to_string().lines().next(),
        Some("your template code is too deeply nested, or the last expression is too complex"),
    );
}

#[test]
fn test_generics_parsing() {
    // Method call.
    Ast::from_str("{{ a.b::<&str, H<B<C>>>() }}", None, &Syntax::default()).unwrap();
    Ast::from_str(
        "{{ a.b::<&str, H<B<C> , &u32>>() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();

    // Call.
    Ast::from_str(
        "{{ a::<&str, H<B<C> , &u32>>() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();

    // Filter.
    Ast::from_str("{{ 12 | a::<&str> }}", None, &Syntax::default()).unwrap();
    Ast::from_str("{{ 12 | a::<&str, u32>('a') }}", None, &Syntax::default()).unwrap();

    // Unclosed `<`.
    assert!(
        Ast::from_str(
            "{{ a.b::<&str, H<B<C> , &u32>() }}",
            None,
            &Syntax::default()
        )
        .is_err()
    );

    // With path and spaces
    Ast::from_str(
        "{{ a.b::<&&core::primitive::str>() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();
    Ast::from_str(
        "{{ a.b ::<&&core::primitive::str>() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();
    Ast::from_str(
        "{{ a.b:: <&&core::primitive::str>() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();
    Ast::from_str(
        "{{ a.b::< &&core::primitive::str>() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();
    Ast::from_str(
        "{{ a.b::<& &core::primitive::str>() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();
    Ast::from_str(
        "{{ a.b::<&& core::primitive::str>() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();
    Ast::from_str(
        "{{ a.b::<&&core ::primitive::str>() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();
    Ast::from_str(
        "{{ a.b::<&&core:: primitive::str>() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();
    Ast::from_str(
        "{{ a.b::<&&core::primitive ::str>() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();
    Ast::from_str(
        "{{ a.b::<&&core::primitive:: str>() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();
    Ast::from_str(
        "{{ a.b::<&&core::primitive::str >() }}",
        None,
        &Syntax::default(),
    )
    .unwrap();
    Ast::from_str(
        "{{ a.b::<&&core::primitive::str> () }}",
        None,
        &Syntax::default(),
    )
    .unwrap();
}

#[test]
fn fuzzed_deeply_tested_if_let() {
    let src = include_str!("../tests/fuzzed-deeply-tested-if-let.txt");
    let syntax = Syntax::default();
    let err = Ast::from_str(src, None, &syntax).unwrap_err();
    assert_eq!(
        err.to_string().lines().next(),
        Some("your template code is too deeply nested, or the last expression is too complex"),
    );
}
