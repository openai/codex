use std::collections::HashSet;
use std::str::{self, FromStr};

use winnow::combinator::{
    alt, cut_err, delimited, empty, eof, fail, not, opt, peek, preceded, repeat, separated,
    terminated,
};
use winnow::stream::Stream as _;
use winnow::token::{any, literal, rest};
use winnow::{ModalParser, Parser};

use crate::memchr_splitter::{Splitter1, Splitter2, Splitter3};
use crate::{
    ErrorContext, Expr, Filter, ParseResult, Span, State, Target, WithSpan, filter, identifier,
    is_rust_keyword, keyword, skip_till, skip_ws0, str_lit_without_prefix, ws,
};

#[derive(Debug, PartialEq)]
pub enum Node<'a> {
    Lit(WithSpan<'a, Lit<'a>>),
    Comment(WithSpan<'a, Comment<'a>>),
    Expr(Ws, WithSpan<'a, Expr<'a>>),
    Call(WithSpan<'a, Call<'a>>),
    Let(WithSpan<'a, Let<'a>>),
    If(WithSpan<'a, If<'a>>),
    Match(WithSpan<'a, Match<'a>>),
    Loop(Box<WithSpan<'a, Loop<'a>>>),
    Extends(WithSpan<'a, Extends<'a>>),
    BlockDef(WithSpan<'a, BlockDef<'a>>),
    Include(WithSpan<'a, Include<'a>>),
    Import(WithSpan<'a, Import<'a>>),
    Macro(WithSpan<'a, Macro<'a>>),
    Raw(WithSpan<'a, Raw<'a>>),
    Break(WithSpan<'a, Ws>),
    Continue(WithSpan<'a, Ws>),
    FilterBlock(WithSpan<'a, FilterBlock<'a>>),
}

impl<'a> Node<'a> {
    pub(super) fn parse_template(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Vec<Self>> {
        let start = *i;
        let result = match (|i: &mut _| Self::many(i, s)).parse_next(i) {
            Ok(result) => result,
            Err(err) => {
                if let winnow::error::ErrMode::Backtrack(err) | winnow::error::ErrMode::Cut(err) =
                    &err
                {
                    if err.message.is_none() {
                        *i = start;
                        if let Some(mut span) = err.span.as_suffix_of(i) {
                            opt(|i: &mut _| unexpected_tag(i, s)).parse_next(&mut span)?;
                        }
                    }
                }
                return Err(err);
            }
        };
        opt(|i: &mut _| unexpected_tag(i, s)).parse_next(i)?;
        let is_eof = opt(eof).parse_next(i)?;
        if is_eof.is_none() {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                "cannot parse entire template\n\
                 you should never encounter this error\n\
                 please report this error to <https://github.com/askama-rs/askama/issues>",
                *i,
            )));
        }
        Ok(result)
    }

    fn many(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Vec<Self>> {
        repeat(
            0..,
            alt((
                |i: &mut _| Lit::parse(i, s).map(Self::Lit),
                |i: &mut _| Comment::parse(i, s).map(Self::Comment),
                |i: &mut _| Self::expr(i, s),
                |i: &mut _| Self::parse(i, s),
            )),
        )
        .map(|v: Vec<_>| v)
        .parse_next(i)
    }

    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Self> {
        let mut start = *i;
        let tag = preceded(
            |i: &mut _| s.tag_block_start(i),
            peek(preceded((opt(Whitespace::parse), skip_ws0), identifier)),
        )
        .parse_next(i)?;

        let func = match tag {
            "call" => |i: &mut _, s| Call::parse(i, s).map(Self::Call),
            "let" | "set" => |i: &mut _, s| Let::parse(i, s).map(Self::Let),
            "if" => |i: &mut _, s| If::parse(i, s).map(Self::If),
            "for" => |i: &mut _, s| Loop::parse(i, s).map(|n| Self::Loop(Box::new(n))),
            "match" => |i: &mut _, s| Match::parse(i, s).map(Self::Match),
            "extends" => |i: &mut _, _s| Extends::parse(i).map(Self::Extends),
            "include" => |i: &mut _, _s| Include::parse(i).map(Self::Include),
            "import" => |i: &mut _, _s| Import::parse(i).map(Self::Import),
            "block" => |i: &mut _, s| BlockDef::parse(i, s).map(Self::BlockDef),
            "macro" => |i: &mut _, s| Macro::parse(i, s).map(Self::Macro),
            "raw" => |i: &mut _, s| Raw::parse(i, s).map(Self::Raw),
            "break" => |i: &mut _, s| Self::r#break(i, s),
            "continue" => |i: &mut _, s| Self::r#continue(i, s),
            "filter" => |i: &mut _, s| FilterBlock::parse(i, s).map(Self::FilterBlock),
            _ => return fail.parse_next(&mut start),
        };

        let _level_guard = s.level.nest(i)?;
        let node = func(i, s)?;
        let closed = cut_node(
            None,
            alt((
                ws(eof).value(false),
                (|i: &mut _| s.tag_block_end(i)).value(true),
            )),
        )
        .parse_next(i)?;
        match closed {
            true => Ok(node),
            false => Err(ErrorContext::unclosed("block", s.syntax.block_end, start).cut()),
        }
    }

    fn r#break(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Self> {
        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("break")),
            opt(Whitespace::parse),
        );

        let start = *i;
        let (pws, _, nws) = p.parse_next(i)?;
        if !s.is_in_loop() {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                "you can only `break` inside a `for` loop",
                start,
            )));
        }
        Ok(Self::Break(WithSpan::new(Ws(pws, nws), start)))
    }

    fn r#continue(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Self> {
        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("continue")),
            opt(Whitespace::parse),
        );

        let start = *i;
        let (pws, _, nws) = p.parse_next(i)?;
        if !s.is_in_loop() {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                "you can only `continue` inside a `for` loop",
                start,
            )));
        }
        Ok(Self::Continue(WithSpan::new(Ws(pws, nws), start)))
    }

    fn expr(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Self> {
        let start = *i;
        let level = s.level;
        let (pws, expr) = preceded(
            |i: &mut _| s.tag_expr_start(i),
            cut_node(
                None,
                (
                    opt(Whitespace::parse),
                    ws(|i: &mut _| Expr::parse(i, level, false)),
                ),
            ),
        )
        .parse_next(i)?;

        let (nws, closed) = cut_node(
            None,
            (
                opt(Whitespace::parse),
                alt((
                    (|i: &mut _| s.tag_expr_end(i)).value(true),
                    ws(eof).value(false),
                )),
            ),
        )
        .parse_next(i)?;
        match closed {
            true => Ok(Self::Expr(Ws(pws, nws), expr)),
            false => Err(ErrorContext::unclosed("expression", s.syntax.expr_end, start).cut()),
        }
    }

    #[must_use]
    pub fn span(&self) -> Span<'a> {
        match self {
            Self::Lit(span) => span.span,
            Self::Comment(span) => span.span,
            Self::Expr(_, span) => span.span,
            Self::Call(span) => span.span,
            Self::Let(span) => span.span,
            Self::If(span) => span.span,
            Self::Match(span) => span.span,
            Self::Loop(span) => span.span,
            Self::Extends(span) => span.span,
            Self::BlockDef(span) => span.span,
            Self::Include(span) => span.span,
            Self::Import(span) => span.span,
            Self::Macro(span) => span.span,
            Self::Raw(span) => span.span,
            Self::Break(span) => span.span,
            Self::Continue(span) => span.span,
            Self::FilterBlock(span) => span.span,
        }
    }
}

fn cut_node<'a, O>(
    kind: Option<&'static str>,
    inner: impl ModalParser<&'a str, O, ErrorContext<'a>>,
) -> impl ModalParser<&'a str, O, ErrorContext<'a>> {
    let mut inner = cut_err(inner);
    move |i: &mut &'a str| {
        let start = *i;
        let result = inner.parse_next(i);
        if let Err(winnow::error::ErrMode::Cut(err) | winnow::error::ErrMode::Backtrack(err)) =
            &result
        {
            if err.message.is_none() {
                *i = start;
                if let Some(mut span) = err.span.as_suffix_of(i) {
                    opt(|i: &mut _| unexpected_raw_tag(kind, i)).parse_next(&mut span)?;
                }
            }
        }
        result
    }
}

fn unexpected_tag<'a>(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, ()> {
    (
        |i: &mut _| s.tag_block_start(i),
        opt(Whitespace::parse),
        |i: &mut _| unexpected_raw_tag(None, i),
    )
        .void()
        .parse_next(i)
}

fn unexpected_raw_tag<'a>(kind: Option<&'static str>, i: &mut &'a str) -> ParseResult<'a, ()> {
    let tag = peek(ws(identifier)).parse_next(i)?;
    let msg = match tag {
        "end" | "elif" | "else" | "when" => match kind {
            Some(kind) => {
                format!("node `{tag}` was not expected in the current context: `{kind}` block")
            }
            None => format!("node `{tag}` was not expected in the current context"),
        },
        tag if tag.starts_with("end") => format!("unexpected closing tag `{tag}`"),
        tag => format!("unknown node `{tag}`"),
    };
    Err(winnow::error::ErrMode::Cut(ErrorContext::new(msg, *i)))
}

#[derive(Debug, PartialEq)]
pub struct When<'a> {
    pub ws: Ws,
    pub target: Vec<Target<'a>>,
    pub nodes: Vec<Node<'a>>,
}

impl<'a> When<'a> {
    fn r#else(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let mut p = (
            |i: &mut _| s.tag_block_start(i),
            opt(Whitespace::parse),
            ws(keyword("else")),
            cut_node(
                Some("match-else"),
                (
                    opt(Whitespace::parse),
                    |i: &mut _| s.tag_block_end(i),
                    cut_node(Some("match-else"), |i: &mut _| Node::many(i, s)),
                ),
            ),
        );

        let start = *i;
        let (_, pws, _, (nws, _, nodes)) = p.parse_next(i)?;
        Ok(WithSpan::new(
            Self {
                ws: Ws(pws, nws),
                target: vec![Target::Placeholder(WithSpan::new((), start))],
                nodes,
            },
            start,
        ))
    }

    #[allow(clippy::self_named_constructors)]
    fn when(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let endwhen = ws((
            delimited(
                |i: &mut _| s.tag_block_start(i),
                opt(Whitespace::parse),
                ws(keyword("endwhen")),
            ),
            cut_node(
                Some("match-endwhen"),
                (
                    opt(Whitespace::parse),
                    |i: &mut _| s.tag_block_end(i),
                    repeat(0.., ws(|i: &mut _| Comment::parse(i, s))).map(|()| ()),
                ),
            ),
        ))
        .with_taken()
        .map(|((pws, _), span)| {
            // A comment node is used to pass the whitespace suppressing information to the
            // generator. This way we don't have to fix up the next `when` node or the closing
            // `endmatch`. Any whitespaces after `endwhen` are to be suppressed. Actually, they
            // don't wind up in the AST anyway.
            Node::Comment(WithSpan::new(
                Comment {
                    ws: Ws(pws, Some(Whitespace::Suppress)),
                    content: "",
                },
                span,
            ))
        });
        let mut p = (
            |i: &mut _| s.tag_block_start(i),
            opt(Whitespace::parse),
            ws(keyword("when")),
            cut_node(
                Some("match-when"),
                (
                    separated(1.., ws(|i: &mut _| Target::parse(i, s)), '|'),
                    opt(Whitespace::parse),
                    |i: &mut _| s.tag_block_end(i),
                    cut_node(Some("match-when"), |i: &mut _| Node::many(i, s)),
                    opt(endwhen),
                ),
            ),
        );
        let (_, pws, _, (target, nws, _, mut nodes, endwhen)) = p.parse_next(i)?;
        if let Some(endwhen) = endwhen {
            nodes.push(endwhen);
        }
        Ok(WithSpan::new(
            Self {
                ws: Ws(pws, nws),
                target,
                nodes,
            },
            start,
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Cond<'a> {
    pub ws: Ws,
    pub cond: Option<CondTest<'a>>,
    pub nodes: Vec<Node<'a>>,
}

impl<'a> Cond<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let (_, pws, cond, nws, _, nodes) = (
            |i: &mut _| s.tag_block_start(i),
            opt(Whitespace::parse),
            alt((
                preceded(ws(keyword("else")), opt(|i: &mut _| CondTest::parse(i, s))),
                preceded(
                    ws(keyword("elif")),
                    cut_node(Some("if-elif"), |i: &mut _| {
                        CondTest::parse_cond(i, s).map(Some)
                    }),
                ),
            )),
            opt(Whitespace::parse),
            cut_node(Some("if"), |i: &mut _| s.tag_block_end(i)),
            cut_node(Some("if"), |i: &mut _| Node::many(i, s)),
        )
            .parse_next(i)?;
        Ok(WithSpan::new(
            Self {
                ws: Ws(pws, nws),
                cond,
                nodes,
            },
            start,
        ))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct CondTest<'a> {
    pub target: Option<Target<'a>>,
    pub expr: WithSpan<'a, Expr<'a>>,
    pub contains_bool_lit_or_is_defined: bool,
}

impl<'a> CondTest<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Self> {
        preceded(
            ws(keyword("if")),
            cut_node(Some("if"), |i: &mut _| Self::parse_cond(i, s)),
        )
        .parse_next(i)
    }

    fn parse_cond(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Self> {
        let (target, expr) = (
            opt(delimited(
                ws(alt((keyword("let"), keyword("set")))),
                ws(|i: &mut _| Target::parse(i, s)),
                ws('='),
            )),
            ws(|i: &mut _| {
                let start = *i;
                let mut expr = Expr::parse(i, s.level, false)?;
                if let Expr::BinOp(_, _, ref mut right) = expr.inner {
                    if matches!(right.inner, Expr::Var("set" | "let")) {
                        let _level_guard = s.level.nest(i)?;
                        *i = right.span.as_suffix_of(start).unwrap();
                        let start_span = Span::from(*i);
                        let new_right = Self::parse_cond(i, s)?;
                        right.inner = Expr::LetCond(Box::new(WithSpan::new(new_right, start_span)));
                    }
                }
                Ok(expr)
            }),
        )
            .parse_next(i)?;
        let contains_bool_lit_or_is_defined = expr.contains_bool_lit_or_is_defined();
        Ok(Self {
            target,
            expr,
            contains_bool_lit_or_is_defined,
        })
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "config", derive(serde_derive::Deserialize))]
#[cfg_attr(feature = "config", serde(field_identifier, rename_all = "lowercase"))]
pub enum Whitespace {
    #[default]
    Preserve,
    Suppress,
    Minimize,
}

impl Whitespace {
    fn parse<'i>(i: &mut &'i str) -> ParseResult<'i, Self> {
        any.verify_map(Self::parse_char).parse_next(i)
    }

    fn parse_char(c: char) -> Option<Self> {
        match c {
            '+' => Some(Self::Preserve),
            '-' => Some(Self::Suppress),
            '~' => Some(Self::Minimize),
            _ => None,
        }
    }
}

impl FromStr for Whitespace {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "+" | "preserve" => Ok(Whitespace::Preserve),
            "-" | "suppress" => Ok(Whitespace::Suppress),
            "~" | "minimize" => Ok(Whitespace::Minimize),
            s => Err(format!("invalid value for `whitespace`: {s:?}")),
        }
    }
}

fn check_block_start<'a>(
    i: &mut &'a str,
    start: &'a str,
    s: &State<'_, '_>,
    node: &str,
    expected: &str,
) -> ParseResult<'a, ()> {
    if i.is_empty() {
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            format!("expected `{expected}` to terminate `{node}` node, found nothing"),
            start,
        )));
    }
    (|i: &mut _| s.tag_block_start(i)).parse_next(i)
}

#[derive(Debug, PartialEq)]
pub struct Loop<'a> {
    pub ws1: Ws,
    pub var: Target<'a>,
    pub iter: WithSpan<'a, Expr<'a>>,
    pub cond: Option<WithSpan<'a, Expr<'a>>>,
    pub body: Vec<Node<'a>>,
    pub ws2: Ws,
    pub else_nodes: Vec<Node<'a>>,
    pub ws3: Ws,
}

impl<'a> Loop<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        fn content<'a>(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Vec<Node<'a>>> {
            s.enter_loop();
            let result = (|i: &mut _| Node::many(i, s)).parse_next(i);
            s.leave_loop();
            result
        }

        let start = *i;
        let if_cond = preceded(
            ws(keyword("if")),
            cut_node(
                Some("for-if"),
                ws(|i: &mut _| Expr::parse(i, s.level, true)),
            ),
        );

        let else_block = |i: &mut _| {
            let mut p = preceded(
                ws(keyword("else")),
                cut_node(
                    Some("for-else"),
                    (
                        opt(Whitespace::parse),
                        delimited(
                            |i: &mut _| s.tag_block_end(i),
                            |i: &mut _| Node::many(i, s),
                            |i: &mut _| s.tag_block_start(i),
                        ),
                        opt(Whitespace::parse),
                    ),
                ),
            );
            let (pws, nodes, nws) = p.parse_next(i)?;
            Ok((pws, nodes, nws))
        };

        let body_and_end = |i: &mut _| {
            let (body, (_, pws, else_block, _, nws)) = cut_node(
                Some("for"),
                (
                    |i: &mut _| content(i, s),
                    cut_node(
                        Some("for"),
                        (
                            |i: &mut _| check_block_start(i, start, s, "for", "endfor"),
                            opt(Whitespace::parse),
                            opt(else_block),
                            end_node("for", "endfor"),
                            opt(Whitespace::parse),
                        ),
                    ),
                ),
            )
            .parse_next(i)?;
            Ok((body, pws, else_block, nws))
        };

        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("for")),
            cut_node(
                Some("for"),
                (
                    ws(|i: &mut _| Target::parse(i, s)),
                    ws(keyword("in")),
                    cut_node(
                        Some("for"),
                        (
                            ws(|i: &mut _| Expr::parse(i, s.level, true)),
                            opt(if_cond),
                            opt(Whitespace::parse),
                            |i: &mut _| s.tag_block_end(i),
                            body_and_end,
                        ),
                    ),
                ),
            ),
        );
        let (pws1, _, (var, _, (iter, cond, nws1, _, (body, pws2, else_block, nws2)))) =
            p.parse_next(i)?;
        let (nws3, else_nodes, pws3) = else_block.unwrap_or_default();
        Ok(WithSpan::new(
            Self {
                ws1: Ws(pws1, nws1),
                var,
                iter,
                cond,
                body,
                ws2: Ws(pws2, nws3),
                else_nodes,
                ws3: Ws(pws3, nws2),
            },
            start,
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Macro<'a> {
    pub ws1: Ws,
    pub name: &'a str,
    pub args: Vec<(&'a str, Option<WithSpan<'a, Expr<'a>>>)>,
    pub nodes: Vec<Node<'a>>,
    pub ws2: Ws,
}

fn check_duplicated_name<'a>(
    names: &mut HashSet<&'a str>,
    arg_name: &'a str,
    i: &'a str,
) -> Result<(), crate::ParseErr<'a>> {
    if !names.insert(arg_name) {
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            format!("duplicated argument `{arg_name}`"),
            i,
        )));
    }
    Ok(())
}

impl<'a> Macro<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let level = s.level;
        #[allow(clippy::type_complexity)]
        let parameters = |i: &mut _| -> ParseResult<
            '_,
            Option<Vec<(&str, Option<WithSpan<'_, Expr<'_>>>)>>,
        > {
            let args = opt(preceded(
                '(',
                (
                    opt(terminated(
                        separated(
                            1..,
                            (
                                ws(identifier),
                                opt(preceded('=', ws(|i: &mut _| Expr::parse(i, level, false)))),
                            ),
                            ',',
                        ),
                        opt(','),
                    )),
                    ws(opt(')')),
                ),
            ))
            .parse_next(i)?;
            match args {
                Some((args, Some(_))) => Ok(args),
                Some((_, None)) => Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    "expected `)` to close macro argument list",
                    *i,
                ))),
                None => Ok(None),
            }
        };

        let start_s = *i;
        let mut start = (
            opt(Whitespace::parse),
            ws(keyword("macro")),
            cut_node(
                Some("macro"),
                (
                    ws(identifier),
                    parameters,
                    opt(Whitespace::parse),
                    |i: &mut _| s.tag_block_end(i),
                ),
            ),
        );
        let (pws1, _, (name, params, nws1, _)) = start.parse_next(i)?;
        if is_rust_keyword(name) {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                format!("'{name}' is not a valid name for a macro"),
                start_s,
            )));
        }

        if let Some(ref params) = params {
            let mut names = HashSet::new();

            let mut iter = params.iter();
            while let Some((arg_name, default_value)) = iter.next() {
                check_duplicated_name(&mut names, arg_name, start_s)?;
                if default_value.is_some() {
                    for (new_arg_name, default_value) in iter.by_ref() {
                        check_duplicated_name(&mut names, new_arg_name, start_s)?;
                        if default_value.is_none() {
                            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                                format!(
                                    "all arguments following `{arg_name}` should have a default \
                                         value, `{new_arg_name}` doesn't have a default value"
                                ),
                                start_s,
                            )));
                        }
                    }
                }
            }
        }

        let mut end = cut_node(
            Some("macro"),
            (
                |i: &mut _| Node::many(i, s),
                cut_node(
                    Some("macro"),
                    (
                        |i: &mut _| check_block_start(i, start_s, s, "macro", "endmacro"),
                        opt(Whitespace::parse),
                        end_node("macro", "endmacro"),
                        cut_node(
                            Some("macro"),
                            preceded(
                                opt(|i: &mut _| {
                                    let before = *i;
                                    let end_name = ws(identifier).parse_next(i)?;
                                    check_end_name(before, name, end_name, "macro")
                                }),
                                opt(Whitespace::parse),
                            ),
                        ),
                    ),
                ),
            ),
        );
        let (contents, (_, pws2, _, nws2)) = end.parse_next(i)?;

        Ok(WithSpan::new(
            Self {
                ws1: Ws(pws1, nws1),
                name,
                args: params.unwrap_or_default(),
                nodes: contents,
                ws2: Ws(pws2, nws2),
            },
            start_s,
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct FilterBlock<'a> {
    pub ws1: Ws,
    pub filters: Filter<'a>,
    pub nodes: Vec<Node<'a>>,
    pub ws2: Ws,
}

impl<'a> FilterBlock<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start_s = *i;
        let mut level_guard = s.level.guard();
        let mut start = (
            opt(Whitespace::parse),
            ws(keyword("filter")),
            cut_node(
                Some("filter"),
                (
                    ws(identifier),
                    opt(|i: &mut _| crate::expr::call_generics(i, s.level)),
                    opt(|i: &mut _| Expr::arguments(i, s.level, false)),
                    repeat(0.., |i: &mut _| {
                        #[allow(clippy::explicit_auto_deref)] // false positive
                        level_guard.nest(*i)?;
                        let start = *i;
                        filter(i, s.level)
                            .map(|(name, generics, params)| (name, generics, params, start))
                    })
                    .map(|v: Vec<_>| v),
                    ws(empty),
                    opt(Whitespace::parse),
                    |i: &mut _| s.tag_block_end(i),
                ),
            ),
        );
        let (pws1, _, (filter_name, generics, params, extra_filters, (), nws1, _)) =
            start.parse_next(i)?;

        let mut arguments = params.unwrap_or_default();
        arguments.insert(0, WithSpan::new(Expr::FilterSource, start_s));
        let mut filters = Filter {
            name: filter_name,
            arguments,
            generics: generics.unwrap_or_default(),
        };
        for (filter_name, generics, args, span) in extra_filters {
            filters = Filter {
                name: filter_name,
                arguments: {
                    let mut args = args.unwrap_or_default();
                    args.insert(0, WithSpan::new(Expr::Filter(filters), span));
                    args
                },
                generics,
            };
        }

        let mut end = cut_node(
            Some("filter"),
            (
                |i: &mut _| Node::many(i, s),
                cut_node(
                    Some("filter"),
                    (
                        |i: &mut _| check_block_start(i, start_s, s, "filter", "endfilter"),
                        opt(Whitespace::parse),
                        end_node("filter", "endfilter"),
                        opt(Whitespace::parse),
                    ),
                ),
            ),
        );
        let (nodes, (_, pws2, _, nws2)) = end.parse_next(i)?;

        Ok(WithSpan::new(
            Self {
                ws1: Ws(pws1, nws1),
                filters,
                nodes,
                ws2: Ws(pws2, nws2),
            },
            start_s,
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Import<'a> {
    pub ws: Ws,
    pub path: &'a str,
    pub scope: &'a str,
}

impl<'a> Import<'a> {
    fn parse(i: &mut &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("import")),
            cut_node(
                Some("import"),
                (
                    ws(str_lit_without_prefix),
                    ws(keyword("as")),
                    cut_node(Some("import"), (ws(identifier), opt(Whitespace::parse))),
                ),
            ),
        );
        let (pws, _, (path, _, (scope, nws))) = p.parse_next(i)?;
        Ok(WithSpan::new(
            Self {
                ws: Ws(pws, nws),
                path,
                scope,
            },
            start,
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Call<'a> {
    pub ws: Ws,
    pub scope: Option<&'a str>,
    pub name: &'a str,
    pub args: Vec<WithSpan<'a, Expr<'a>>>,
}

impl<'a> Call<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("call")),
            cut_node(
                Some("call"),
                (
                    opt((ws(identifier), ws("::"))),
                    ws(identifier),
                    opt(ws(|nested: &mut _| Expr::arguments(nested, s.level, true))),
                    opt(Whitespace::parse),
                ),
            ),
        );
        let (pws, _, (scope, name, args, nws)) = p.parse_next(i)?;
        let scope = scope.map(|(scope, _)| scope);
        let args = args.unwrap_or_default();
        Ok(WithSpan::new(
            Self {
                ws: Ws(pws, nws),
                scope,
                name,
                args,
            },
            start,
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Match<'a> {
    pub ws1: Ws,
    pub expr: WithSpan<'a, Expr<'a>>,
    pub arms: Vec<WithSpan<'a, When<'a>>>,
    pub ws2: Ws,
}

impl<'a> Match<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("match")),
            cut_node(
                Some("match"),
                (
                    ws(|i: &mut _| Expr::parse(i, s.level, false)),
                    opt(Whitespace::parse),
                    |i: &mut _| s.tag_block_end(i),
                    cut_node(
                        Some("match"),
                        (
                            ws(repeat(0.., ws(|i: &mut _| Comment::parse(i, s)))).map(|()| ()),
                            repeat(0.., |i: &mut _| When::when(i, s)).map(|v: Vec<_>| v),
                            cut_node(
                                Some("match"),
                                (
                                    opt(|i: &mut _| When::r#else(i, s)),
                                    cut_node(
                                        Some("match"),
                                        (
                                            ws(|i: &mut _| {
                                                check_block_start(i, start, s, "match", "endmatch")
                                            }),
                                            opt(Whitespace::parse),
                                            end_node("match", "endmatch"),
                                            opt(Whitespace::parse),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        );
        let (pws1, _, (expr, nws1, _, (_, mut arms, (else_arm, (_, pws2, _, nws2))))) =
            p.parse_next(i)?;

        if let Some(arm) = else_arm {
            arms.push(arm);
        }
        if arms.is_empty() {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                "`match` nodes must contain at least one `when` node and/or an `else` case",
                start,
            )));
        }

        Ok(WithSpan::new(
            Self {
                ws1: Ws(pws1, nws1),
                expr,
                arms,
                ws2: Ws(pws2, nws2),
            },
            start,
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct BlockDef<'a> {
    pub ws1: Ws,
    pub name: &'a str,
    pub nodes: Vec<Node<'a>>,
    pub ws2: Ws,
}

impl<'a> BlockDef<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start_s = *i;
        let mut start = (
            opt(Whitespace::parse),
            ws(keyword("block")),
            cut_node(
                Some("block"),
                (ws(identifier), opt(Whitespace::parse), |i: &mut _| {
                    s.tag_block_end(i)
                }),
            ),
        );
        let (pws1, _, (name, nws1, _)) = start.parse_next(i)?;

        let mut end = cut_node(
            Some("block"),
            (
                |i: &mut _| Node::many(i, s),
                cut_node(
                    Some("block"),
                    (
                        |i: &mut _| check_block_start(i, start_s, s, "block", "endblock"),
                        opt(Whitespace::parse),
                        end_node("block", "endblock"),
                        cut_node(
                            Some("block"),
                            (
                                opt(|i: &mut _| {
                                    let before = *i;
                                    let end_name = ws(identifier).parse_next(i)?;
                                    check_end_name(before, name, end_name, "block")
                                }),
                                opt(Whitespace::parse),
                            ),
                        ),
                    ),
                ),
            ),
        );
        let (nodes, (_, pws2, _, (_, nws2))) = end.parse_next(i)?;

        Ok(WithSpan::new(
            BlockDef {
                ws1: Ws(pws1, nws1),
                name,
                nodes,
                ws2: Ws(pws2, nws2),
            },
            start_s,
        ))
    }
}

fn check_end_name<'a>(
    before: &'a str,
    name: &'a str,
    end_name: &'a str,
    kind: &str,
) -> ParseResult<'a> {
    if name == end_name {
        return Ok(end_name);
    }

    Err(winnow::error::ErrMode::Cut(ErrorContext::new(
        match name.is_empty() && !end_name.is_empty() {
            true => format!("unexpected name `{end_name}` in `end{kind}` tag for unnamed `{kind}`"),
            false => format!("expected name `{name}` in `end{kind}` tag, found `{end_name}`"),
        },
        before,
    )))
}

#[derive(Debug, PartialEq)]
pub struct Lit<'a> {
    pub lws: &'a str,
    pub val: &'a str,
    pub rws: &'a str,
}

impl<'a> Lit<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        not(eof).parse_next(i)?;

        let candidate_finder = Splitter3::new(
            s.syntax.block_start,
            s.syntax.comment_start,
            s.syntax.expr_start,
        );
        let p_start = alt((
            literal(s.syntax.block_start),
            literal(s.syntax.comment_start),
            literal(s.syntax.expr_start),
        ));

        let content = opt(skip_till(candidate_finder, p_start).take()).parse_next(i)?;
        let content = match content {
            Some("") => {
                // {block,comment,expr}_start follows immediately.
                return fail.parse_next(i);
            }
            Some(content) => content,
            None => rest.parse_next(i)?, /* there is no {block,comment,expr}_start: take everything */
        };
        Ok(WithSpan::new(Self::split_ws_parts(content), start))
    }

    pub(crate) fn split_ws_parts(s: &'a str) -> Self {
        let trimmed_start = s.trim_ascii_start();
        let len_start = s.len() - trimmed_start.len();
        let trimmed = trimmed_start.trim_ascii_end();
        Self {
            lws: &s[..len_start],
            val: trimmed,
            rws: &trimmed_start[trimmed.len()..],
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Raw<'a> {
    pub ws1: Ws,
    pub lit: Lit<'a>,
    pub ws2: Ws,
}

impl<'a> Raw<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let endraw = (
            |i: &mut _| s.tag_block_start(i),
            opt(Whitespace::parse),
            ws(keyword("endraw")), // sic: ignore `{% end %}` in raw blocks
            opt(Whitespace::parse),
            peek(|i: &mut _| s.tag_block_end(i)),
        );

        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("raw")),
            cut_node(
                Some("raw"),
                (
                    opt(Whitespace::parse),
                    |i: &mut _| s.tag_block_end(i),
                    skip_till(Splitter1::new(s.syntax.block_start), endraw).with_taken(),
                ),
            ),
        );

        let (pws1, _, (nws1, _, ((new_i, (_, pws2, _, nws2, _)), contents))) = p.parse_next(i)?;
        *i = new_i;
        let lit = Lit::split_ws_parts(contents);
        let ws1 = Ws(pws1, nws1);
        let ws2 = Ws(pws2, nws2);
        Ok(WithSpan::new(Self { ws1, lit, ws2 }, start))
    }
}

#[derive(Debug, PartialEq)]
pub struct Let<'a> {
    pub ws: Ws,
    pub var: Target<'a>,
    pub val: Option<WithSpan<'a, Expr<'a>>>,
}

impl<'a> Let<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let mut p = (
            opt(Whitespace::parse),
            ws(alt((keyword("let"), keyword("set")))),
            cut_node(
                Some("let"),
                (
                    ws(|i: &mut _| Target::parse(i, s)),
                    opt(preceded(
                        ws('='),
                        ws(|i: &mut _| Expr::parse(i, s.level, false)),
                    )),
                    opt(Whitespace::parse),
                ),
            ),
        );
        let (pws, _, (var, val, nws)) = p.parse_next(i)?;
        if val.is_none() {
            let kind = match &var {
                Target::Name(_) => None,
                Target::Tuple(..) => Some("a tuple"),
                Target::Array(..) => Some("an array"),
                Target::Struct(..) => Some("a struct"),
                Target::NumLit(..)
                | Target::StrLit(..)
                | Target::CharLit(..)
                | Target::BoolLit(..) => Some("a literal"),
                Target::Path(..) => Some("a path or enum variant"),
                Target::OrChain(..) | Target::Placeholder(..) | Target::Rest(..) => {
                    Some("a pattern")
                }
            };
            if let Some(kind) = kind {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    format!(
                        "when you forward-define a variable, you cannot use {kind} in place of \
                         a variable name"
                    ),
                    start,
                )));
            }
        }

        Ok(WithSpan::new(
            Let {
                ws: Ws(pws, nws),
                var,
                val,
            },
            start,
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct If<'a> {
    pub ws: Ws,
    pub branches: Vec<WithSpan<'a, Cond<'a>>>,
}

impl<'a> If<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let mut p = (
            opt(Whitespace::parse),
            |i: &mut _| CondTest::parse(i, s),
            cut_node(
                Some("if"),
                (
                    opt(Whitespace::parse),
                    |i: &mut _| s.tag_block_end(i),
                    cut_node(
                        Some("if"),
                        (
                            |i: &mut _| Node::many(i, s),
                            repeat(0.., |i: &mut _| Cond::parse(i, s)).map(|v: Vec<_>| v),
                            cut_node(
                                Some("if"),
                                (
                                    |i: &mut _| check_block_start(i, start, s, "if", "endif"),
                                    opt(Whitespace::parse),
                                    end_node("if", "endif"),
                                    opt(Whitespace::parse),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        );

        let (pws1, cond, (nws1, _, (nodes, elifs, (_, pws2, _, nws2)))) = p.parse_next(i)?;
        let mut branches = vec![WithSpan::new(
            Cond {
                ws: Ws(pws1, nws1),
                cond: Some(cond),
                nodes,
            },
            start,
        )];
        branches.extend(elifs);

        Ok(WithSpan::new(
            Self {
                ws: Ws(pws2, nws2),
                branches,
            },
            start,
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Include<'a> {
    pub ws: Ws,
    pub path: &'a str,
}

impl<'a> Include<'a> {
    fn parse(i: &mut &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("include")),
            cut_node(
                Some("include"),
                (ws(str_lit_without_prefix), opt(Whitespace::parse)),
            ),
        );
        let (pws, _, (path, nws)) = p.parse_next(i)?;
        Ok(WithSpan::new(
            Self {
                ws: Ws(pws, nws),
                path,
            },
            start,
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Extends<'a> {
    pub path: &'a str,
}

impl<'a> Extends<'a> {
    fn parse(i: &mut &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        preceded(
            (opt(Whitespace::parse), ws(keyword("extends"))),
            cut_node(
                Some("extends"),
                terminated(ws(str_lit_without_prefix), opt(Whitespace::parse)),
            ),
        )
        .map(|path| WithSpan::new(Self { path }, start))
        .parse_next(i)
    }
}

#[derive(Debug, PartialEq)]
pub struct Comment<'a> {
    pub ws: Ws,
    pub content: &'a str,
}

impl<'a> Comment<'a> {
    fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        #[derive(Debug, Clone, Copy)]
        enum Tag {
            Open,
            Close,
        }

        fn tag<'a>(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Tag> {
            alt((
                (|i: &mut _| s.tag_comment_start(i)).value(Tag::Open),
                (|i: &mut _| s.tag_comment_end(i)).value(Tag::Close),
            ))
            .parse_next(i)
        }

        fn content<'a>(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a> {
            let mut depth = 0usize;
            let start = *i;
            loop {
                let splitter = Splitter2::new(s.syntax.comment_start, s.syntax.comment_end);
                let tag = opt(skip_till(splitter, |i: &mut _| tag(i, s))).parse_next(i)?;
                let Some((inclusive, tag)) = tag else {
                    return Err(
                        ErrorContext::unclosed("comment", s.syntax.comment_end, start).cut(),
                    );
                };
                match tag {
                    Tag::Open => match depth.checked_add(1) {
                        Some(new_depth) => depth = new_depth,
                        None => {
                            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                                "too deeply nested comments",
                                start,
                            )));
                        }
                    },
                    Tag::Close => match depth.checked_sub(1) {
                        Some(new_depth) => depth = new_depth,
                        None => {
                            let exclusive = *i;
                            *i = inclusive;
                            return Ok(&start[..start.len() - exclusive.len()]);
                        }
                    },
                }
                *i = inclusive;
            }
        }

        let start = *i;
        let content = preceded(
            |i: &mut _| s.tag_comment_start(i),
            cut_node(Some("comment"), |i: &mut _| content(i, s)),
        )
        .parse_next(i)?;

        let mut ws = Ws(None, None);
        if content.len() == 1 && matches!(content, "-" | "+" | "~") {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                format!(
                    "ambiguous whitespace stripping\n\
                     use `{}{content} {content}{}` to apply the same whitespace stripping on both \
                     sides",
                    s.syntax.comment_start, s.syntax.comment_end,
                ),
                start,
            )));
        } else if content.len() >= 2 {
            ws.0 = Whitespace::parse_char(content.chars().next().unwrap_or_default());
            ws.1 = Whitespace::parse_char(content.chars().next_back().unwrap_or_default());
        }

        Ok(WithSpan::new(Self { ws, content }, start))
    }
}

/// First field is "minus/plus sign was used on the left part of the item".
///
/// Second field is "minus/plus sign was used on the right part of the item".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ws(pub Option<Whitespace>, pub Option<Whitespace>);

fn end_node<'a, 'g: 'a>(
    node: &'g str,
    expected: &'g str,
) -> impl ModalParser<&'a str, &'a str, ErrorContext<'a>> + 'g {
    move |i: &mut &'a str| {
        let start = i.checkpoint();
        let actual = ws(identifier).parse_next(i)?;
        if actual == expected {
            Ok(actual)
        } else if actual.starts_with("end") {
            i.reset(&start);
            Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                format!("expected `{expected}` to terminate `{node}` node, found `{actual}`"),
                *i,
            )))
        } else {
            i.reset(&start);
            fail.parse_next(i)
        }
    }
}
