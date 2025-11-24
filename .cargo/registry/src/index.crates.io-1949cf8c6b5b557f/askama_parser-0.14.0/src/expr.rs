use std::collections::HashSet;
use std::str;

use winnow::Parser;
use winnow::ascii::digit1;
use winnow::combinator::{
    alt, cut_err, fail, not, opt, peek, preceded, repeat, separated, terminated,
};
use winnow::error::ParserError as _;
use winnow::stream::Stream as _;

use crate::node::CondTest;
use crate::{
    CharLit, ErrorContext, Level, Num, ParseErr, ParseResult, PathOrIdentifier, Span, StrLit,
    WithSpan, char_lit, filter, identifier, keyword, num_lit, path_or_identifier, skip_ws0,
    skip_ws1, str_lit, ws,
};

macro_rules! expr_prec_layer {
    ( $name:ident, $inner:ident, $op:expr ) => {
        fn $name(i: &mut &'a str, level: Level<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
            let mut level_guard = level.guard();
            let start = *i;
            let mut expr = Self::$inner(i, level)?;
            let mut i_before = *i;
            let mut right = opt((ws($op), |i: &mut _| Self::$inner(i, level)));
            while let Some((op, right)) = right.parse_next(i)? {
                level_guard.nest(i_before)?;
                i_before = *i;
                expr = WithSpan::new(Self::BinOp(op, Box::new(expr), Box::new(right)), start);
            }
            Ok(expr)
        }
    };
}

fn check_expr<'a>(
    expr: &WithSpan<'a, Expr<'a>>,
    allow_underscore: bool,
) -> Result<(), ParseErr<'a>> {
    match &expr.inner {
        Expr::Var("_") if !allow_underscore => Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "reserved keyword `_` cannot be used here",
            expr.span,
        ))),
        Expr::IsDefined(var) | Expr::IsNotDefined(var) => {
            if *var == "_" {
                Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    "reserved keyword `_` cannot be used here",
                    expr.span,
                )))
            } else {
                Ok(())
            }
        }
        Expr::BoolLit(_)
        | Expr::NumLit(_, _)
        | Expr::StrLit(_)
        | Expr::CharLit(_)
        | Expr::Path(_)
        | Expr::Attr(_, _)
        | Expr::Filter(_)
        | Expr::NamedArgument(_, _)
        | Expr::Var(_)
        | Expr::RustMacro(_, _)
        | Expr::Try(_)
        | Expr::FilterSource
        | Expr::LetCond(_) => Ok(()),
        Expr::Array(elems) | Expr::Tuple(elems) | Expr::Concat(elems) => {
            for elem in elems {
                check_expr(elem, allow_underscore)?;
            }
            Ok(())
        }
        Expr::Index(elem1, elem2) | Expr::BinOp(_, elem1, elem2) => {
            check_expr(elem1, false)?;
            check_expr(elem2, false)
        }
        Expr::Range(_, elem1, elem2) => {
            if let Some(elem1) = elem1 {
                check_expr(elem1, false)?;
            }
            if let Some(elem2) = elem2 {
                check_expr(elem2, false)?;
            }
            Ok(())
        }
        Expr::As(elem, _) | Expr::Unary(_, elem) | Expr::Group(elem) => check_expr(elem, false),
        Expr::Call { path, args, .. } => {
            check_expr(path, false)?;
            for arg in args {
                check_expr(arg, false)?;
            }
            Ok(())
        }
        Expr::ArgumentPlaceholder => Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "unreachable",
            expr.span,
        ))),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr<'a> {
    BoolLit(bool),
    NumLit(&'a str, Num<'a>),
    StrLit(StrLit<'a>),
    CharLit(CharLit<'a>),
    Var(&'a str),
    Path(Vec<&'a str>),
    Array(Vec<WithSpan<'a, Expr<'a>>>),
    Attr(Box<WithSpan<'a, Expr<'a>>>, Attr<'a>),
    Index(Box<WithSpan<'a, Expr<'a>>>, Box<WithSpan<'a, Expr<'a>>>),
    Filter(Filter<'a>),
    As(Box<WithSpan<'a, Expr<'a>>>, &'a str),
    NamedArgument(&'a str, Box<WithSpan<'a, Expr<'a>>>),
    Unary(&'a str, Box<WithSpan<'a, Expr<'a>>>),
    BinOp(
        &'a str,
        Box<WithSpan<'a, Expr<'a>>>,
        Box<WithSpan<'a, Expr<'a>>>,
    ),
    Range(
        &'a str,
        Option<Box<WithSpan<'a, Expr<'a>>>>,
        Option<Box<WithSpan<'a, Expr<'a>>>>,
    ),
    Group(Box<WithSpan<'a, Expr<'a>>>),
    Tuple(Vec<WithSpan<'a, Expr<'a>>>),
    Call {
        path: Box<WithSpan<'a, Expr<'a>>>,
        args: Vec<WithSpan<'a, Expr<'a>>>,
        generics: Vec<WithSpan<'a, TyGenerics<'a>>>,
    },
    RustMacro(Vec<&'a str>, &'a str),
    Try(Box<WithSpan<'a, Expr<'a>>>),
    /// This variant should never be used directly. It is created when generating filter blocks.
    FilterSource,
    IsDefined(&'a str),
    IsNotDefined(&'a str),
    Concat(Vec<WithSpan<'a, Expr<'a>>>),
    /// If you have `&& let Some(y)`, this variant handles it.
    LetCond(Box<WithSpan<'a, CondTest<'a>>>),
    /// This variant should never be used directly.
    /// It is used for the handling of named arguments in the generator, esp. with filters.
    ArgumentPlaceholder,
}

impl<'a> Expr<'a> {
    pub(super) fn arguments(
        i: &mut &'a str,
        level: Level<'_>,
        is_template_macro: bool,
    ) -> ParseResult<'a, Vec<WithSpan<'a, Self>>> {
        let _level_guard = level.nest(i)?;
        let mut named_arguments = HashSet::new();
        let start = *i;

        preceded(
            ws('('),
            cut_err(terminated(
                separated(
                    0..,
                    ws(move |i: &mut _| {
                        // Needed to prevent borrowing it twice between this closure and the one
                        // calling `Self::named_arguments`.
                        let named_arguments = &mut named_arguments;
                        let has_named_arguments = !named_arguments.is_empty();

                        let expr = alt((
                            move |i: &mut _| {
                                Self::named_argument(
                                    i,
                                    level,
                                    named_arguments,
                                    start,
                                    is_template_macro,
                                )
                            },
                            move |i: &mut _| Self::parse(i, level, false),
                        ))
                        .parse_next(i)?;
                        if has_named_arguments && !matches!(*expr, Self::NamedArgument(_, _)) {
                            Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                                "named arguments must always be passed last",
                                start,
                            )))
                        } else {
                            Ok(expr)
                        }
                    }),
                    ',',
                ),
                (opt(ws(',')), ')'),
            )),
        )
        .parse_next(i)
    }

    fn named_argument(
        i: &mut &'a str,
        level: Level<'_>,
        named_arguments: &mut HashSet<&'a str>,
        start: &'a str,
        is_template_macro: bool,
    ) -> ParseResult<'a, WithSpan<'a, Self>> {
        if !is_template_macro {
            // If this is not a template macro, we don't want to parse named arguments so
            // we instead return an error which will allow to continue the parsing.
            return fail.parse_next(i);
        }

        let (argument, _, value) = (identifier, ws('='), move |i: &mut _| {
            Self::parse(i, level, false)
        })
            .parse_next(i)?;
        if named_arguments.insert(argument) {
            Ok(WithSpan::new(
                Self::NamedArgument(argument, Box::new(value)),
                start,
            ))
        } else {
            Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                format!("named argument `{argument}` was passed more than once"),
                start,
            )))
        }
    }

    pub(super) fn parse(
        i: &mut &'a str,
        level: Level<'_>,
        allow_underscore: bool,
    ) -> ParseResult<'a, WithSpan<'a, Self>> {
        let _level_guard = level.nest(i)?;
        let start = Span::from(*i);
        let range_right = move |i: &mut _| {
            (
                ws(alt(("..=", ".."))),
                opt(move |i: &mut _| Self::or(i, level)),
            )
                .parse_next(i)
        };
        let expr = alt((
            range_right.map(move |(op, right)| {
                WithSpan::new(Self::Range(op, None, right.map(Box::new)), start)
            }),
            (move |i: &mut _| Self::or(i, level), opt(range_right)).map(move |(left, right)| {
                match right {
                    Some((op, right)) => WithSpan::new(
                        Self::Range(op, Some(Box::new(left)), right.map(Box::new)),
                        start,
                    ),
                    None => left,
                }
            }),
        ))
        .parse_next(i)?;
        check_expr(&expr, allow_underscore)?;
        Ok(expr)
    }

    expr_prec_layer!(or, and, "||");
    expr_prec_layer!(and, compare, "&&");
    expr_prec_layer!(compare, bor, alt(("==", "!=", ">=", ">", "<=", "<",)));
    expr_prec_layer!(bor, bxor, "bitor".value("|"));
    expr_prec_layer!(bxor, band, token_xor);
    expr_prec_layer!(band, shifts, token_bitand);
    expr_prec_layer!(shifts, addsub, alt((">>", "<<")));
    expr_prec_layer!(addsub, concat, alt(("+", "-")));

    fn concat(i: &mut &'a str, level: Level<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        fn concat_expr<'a>(
            i: &mut &'a str,
            level: Level<'_>,
        ) -> ParseResult<'a, Option<WithSpan<'a, Expr<'a>>>> {
            let ws1 = |i: &mut _| opt(skip_ws1).parse_next(i);

            let start = *i;
            let data = opt((ws1, '~', ws1, |i: &mut _| Expr::muldivmod(i, level))).parse_next(i)?;
            if let Some((t1, _, t2, expr)) = data {
                if t1.is_none() || t2.is_none() {
                    return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                        "the concat operator `~` must be surrounded by spaces",
                        start,
                    )));
                }
                Ok(Some(expr))
            } else {
                Ok(None)
            }
        }

        let start = *i;
        let expr = Self::muldivmod(i, level)?;
        let expr2 = concat_expr(i, level)?;
        if let Some(expr2) = expr2 {
            let mut exprs = vec![expr, expr2];
            while let Some(expr) = concat_expr(i, level)? {
                exprs.push(expr);
            }
            Ok(WithSpan::new(Self::Concat(exprs), start))
        } else {
            Ok(expr)
        }
    }

    expr_prec_layer!(muldivmod, is_as, alt(("*", "/", "%")));

    fn is_as(i: &mut &'a str, level: Level<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let lhs = Self::filtered(i, level)?;
        let before_keyword = *i;
        let rhs = opt(ws(identifier)).parse_next(i)?;
        match rhs {
            Some("is") => {}
            Some("as") => {
                let target = opt(identifier).parse_next(i)?;
                let target = target.unwrap_or_default();
                if crate::PRIMITIVE_TYPES.contains(&target) {
                    return Ok(WithSpan::new(Self::As(Box::new(lhs), target), start));
                } else if target.is_empty() {
                    return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                        "`as` operator expects the name of a primitive type on its right-hand side",
                        before_keyword.trim_start(),
                    )));
                } else {
                    return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                        format!(
                            "`as` operator expects the name of a primitive type on its right-hand \
                              side, found `{target}`"
                        ),
                        before_keyword.trim_start(),
                    )));
                }
            }
            _ => {
                *i = before_keyword;
                return Ok(lhs);
            }
        }

        let rhs = opt(terminated(opt(keyword("not")), ws(keyword("defined")))).parse_next(i)?;
        let ctor = match rhs {
            None => {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    "expected `defined` or `not defined` after `is`",
                    // We use `start` to show the whole `var is` thing instead of the current token.
                    start,
                )));
            }
            Some(None) => Self::IsDefined,
            Some(Some(_)) => Self::IsNotDefined,
        };
        let var_name = match *lhs {
            Self::Var(var_name) => var_name,
            Self::Attr(_, _) => {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    "`is defined` operator can only be used on variables, not on their fields",
                    start,
                )));
            }
            _ => {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    "`is defined` operator can only be used on variables",
                    start,
                )));
            }
        };
        Ok(WithSpan::new(ctor(var_name), start))
    }

    fn filtered(i: &mut &'a str, level: Level<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let mut level_guard = level.guard();
        let start = *i;
        let mut res = Self::prefix(i, level)?;
        while let Some((name, generics, args)) = opt(|i: &mut _| filter(i, level)).parse_next(i)? {
            level_guard.nest(i)?;

            let mut arguments = args.unwrap_or_else(|| Vec::with_capacity(1));
            arguments.insert(0, res);

            res = WithSpan::new(
                Self::Filter(Filter {
                    name,
                    arguments,
                    generics,
                }),
                start,
            );
        }
        Ok(res)
    }

    fn prefix(i: &mut &'a str, level: Level<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;

        // This is a rare place where we create recursion in the parsed AST
        // without recursing the parser call stack. However, this can lead
        // to stack overflows in drop glue when the AST is very deep.
        let mut level_guard = level.guard();
        let mut ops = vec![];
        let mut i_before = *i;
        while let Some(op) = opt(ws(alt(("!", "-", "*", "&")))).parse_next(i)? {
            level_guard.nest(i_before)?;
            ops.push(op);
            i_before = *i;
        }

        let mut expr = Suffix::parse(i, level)?;
        for op in ops.iter().rev() {
            expr = WithSpan::new(Self::Unary(op, Box::new(expr)), start);
        }

        Ok(expr)
    }

    fn single(i: &mut &'a str, level: Level<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        alt((
            Self::num,
            Self::str,
            Self::char,
            Self::path_var_bool,
            move |i: &mut _| Self::array(i, level),
            move |i: &mut _| Self::group(i, level),
        ))
        .parse_next(i)
    }

    fn group(i: &mut &'a str, level: Level<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let expr = preceded(ws('('), opt(|i: &mut _| Self::parse(i, level, true))).parse_next(i)?;
        let Some(expr) = expr else {
            let _ = ')'.parse_next(i)?;
            return Ok(WithSpan::new(Self::Tuple(vec![]), start));
        };

        let comma = ws(opt(peek(','))).parse_next(i)?;
        if comma.is_none() {
            let _ = ')'.parse_next(i)?;
            return Ok(WithSpan::new(Self::Group(Box::new(expr)), start));
        }

        let mut exprs = vec![expr];
        repeat(
            0..,
            preceded(',', ws(|i: &mut _| Self::parse(i, level, true))),
        )
        .fold(
            || (),
            |(), expr| {
                exprs.push(expr);
            },
        )
        .parse_next(i)?;
        let _ = (ws(opt(',')), ')').parse_next(i)?;
        Ok(WithSpan::new(Self::Tuple(exprs), start))
    }

    fn array(i: &mut &'a str, level: Level<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let array = preceded(
            ws('['),
            cut_err(terminated(
                opt(terminated(
                    separated(1.., ws(move |i: &mut _| Self::parse(i, level, true)), ','),
                    ws(opt(',')),
                )),
                ']',
            )),
        )
        .parse_next(i)?;
        Ok(WithSpan::new(Self::Array(array.unwrap_or_default()), start))
    }

    fn path_var_bool(i: &mut &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        path_or_identifier
            .map(|v| match v {
                PathOrIdentifier::Path(v) => Self::Path(v),
                PathOrIdentifier::Identifier("true") => Self::BoolLit(true),
                PathOrIdentifier::Identifier("false") => Self::BoolLit(false),
                PathOrIdentifier::Identifier(v) => Self::Var(v),
            })
            .parse_next(i)
            .map(|expr| WithSpan::new(expr, start))
    }

    fn str(i: &mut &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        str_lit
            .map(|i| WithSpan::new(Self::StrLit(i), start))
            .parse_next(i)
    }

    fn num(i: &mut &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let (num, full) = num_lit.with_taken().parse_next(i)?;
        Ok(WithSpan::new(Expr::NumLit(full, num), start))
    }

    fn char(i: &mut &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        char_lit
            .map(|i| WithSpan::new(Self::CharLit(i), start))
            .parse_next(i)
    }

    #[must_use]
    pub fn contains_bool_lit_or_is_defined(&self) -> bool {
        match self {
            Self::BoolLit(_) | Self::IsDefined(_) | Self::IsNotDefined(_) => true,
            Self::Unary(_, expr) | Self::Group(expr) => expr.contains_bool_lit_or_is_defined(),
            Self::BinOp("&&" | "||", left, right) => {
                left.contains_bool_lit_or_is_defined() || right.contains_bool_lit_or_is_defined()
            }
            Self::NumLit(_, _)
            | Self::StrLit(_)
            | Self::CharLit(_)
            | Self::Var(_)
            | Self::FilterSource
            | Self::RustMacro(_, _)
            | Self::As(_, _)
            | Self::Call { .. }
            | Self::Range(_, _, _)
            | Self::Try(_)
            | Self::NamedArgument(_, _)
            | Self::Filter(_)
            | Self::Attr(_, _)
            | Self::Index(_, _)
            | Self::Tuple(_)
            | Self::Array(_)
            | Self::BinOp(_, _, _)
            | Self::Path(_)
            | Self::Concat(_)
            | Self::LetCond(_)
            | Self::ArgumentPlaceholder => false,
        }
    }
}

fn token_xor<'a>(i: &mut &'a str) -> ParseResult<'a> {
    let good = alt((keyword("xor").value(true), '^'.value(false))).parse_next(i)?;
    if good {
        Ok("^")
    } else {
        Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "the binary XOR operator is called `xor` in askama",
            *i,
        )))
    }
}

fn token_bitand<'a>(i: &mut &'a str) -> ParseResult<'a> {
    let good = alt((keyword("bitand").value(true), ('&', not('&')).value(false))).parse_next(i)?;
    if good {
        Ok("&")
    } else {
        Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "the binary AND operator is called `bitand` in askama",
            *i,
        )))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Filter<'a> {
    pub name: &'a str,
    pub arguments: Vec<WithSpan<'a, Expr<'a>>>,
    pub generics: Vec<WithSpan<'a, TyGenerics<'a>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attr<'a> {
    pub name: &'a str,
    pub generics: Vec<WithSpan<'a, TyGenerics<'a>>>,
}

enum Suffix<'a> {
    Attr(Attr<'a>),
    Index(WithSpan<'a, Expr<'a>>),
    Call {
        args: Vec<WithSpan<'a, Expr<'a>>>,
        generics: Vec<WithSpan<'a, TyGenerics<'a>>>,
    },
    // The value is the arguments of the macro call.
    MacroCall(&'a str),
    Try,
}

impl<'a> Suffix<'a> {
    fn parse(i: &mut &'a str, level: Level<'_>) -> ParseResult<'a, WithSpan<'a, Expr<'a>>> {
        let mut level_guard = level.guard();
        let mut expr = Expr::single(i, level)?;
        let mut right = opt(alt((
            |i: &mut _| Self::attr(i, level),
            |i: &mut _| Self::index(i, level),
            |i: &mut _| Self::call(i, level),
            Self::r#try,
            Self::r#macro,
        )));
        loop {
            let before_suffix = *i;
            let suffix = right.parse_next(i)?;
            let Some(suffix) = suffix else {
                break;
            };
            level_guard.nest(before_suffix)?;

            match suffix {
                Self::Attr(attr) => {
                    expr = WithSpan::new(Expr::Attr(expr.into(), attr), before_suffix)
                }
                Self::Index(index) => {
                    expr = WithSpan::new(Expr::Index(expr.into(), index.into()), before_suffix);
                }
                Self::Call { args, generics } => {
                    expr = WithSpan::new(
                        Expr::Call {
                            path: expr.into(),
                            args,
                            generics,
                        },
                        before_suffix,
                    )
                }
                Self::Try => expr = WithSpan::new(Expr::Try(expr.into()), before_suffix),
                Self::MacroCall(args) => match expr.inner {
                    Expr::Path(path) => {
                        expr = WithSpan::new(Expr::RustMacro(path, args), before_suffix)
                    }
                    Expr::Var(name) => {
                        expr = WithSpan::new(Expr::RustMacro(vec![name], args), before_suffix)
                    }
                    _ => {
                        return Err(winnow::error::ErrMode::from_input(&before_suffix).cut());
                    }
                },
            }
        }
        Ok(expr)
    }

    fn r#macro(i: &mut &'a str) -> ParseResult<'a, Self> {
        fn nested_parenthesis<'a>(input: &mut &'a str) -> ParseResult<'a, ()> {
            let mut nested = 0;
            let mut last = 0;
            let mut in_str = false;
            let mut escaped = false;

            for (i, c) in input.char_indices() {
                if !(c == '(' || c == ')') || !in_str {
                    match c {
                        '(' => nested += 1,
                        ')' => {
                            if nested == 0 {
                                last = i;
                                break;
                            }
                            nested -= 1;
                        }
                        '"' => {
                            if in_str {
                                if !escaped {
                                    in_str = false;
                                }
                            } else {
                                in_str = true;
                            }
                        }
                        '\\' => {
                            escaped = !escaped;
                        }
                        _ => (),
                    }
                }

                if escaped && c != '\\' {
                    escaped = false;
                }
            }

            if nested == 0 {
                let _ = input.next_slice(last);
                Ok(())
            } else {
                fail.parse_next(input)
            }
        }

        preceded(
            (ws('!'), '('),
            cut_err(terminated(
                nested_parenthesis.take().map(Self::MacroCall),
                ')',
            )),
        )
        .parse_next(i)
    }

    fn attr(i: &mut &'a str, level: Level<'_>) -> ParseResult<'a, Self> {
        preceded(
            ws(('.', not('.'))),
            cut_err((
                alt((digit1, identifier)),
                opt(|i: &mut _| call_generics(i, level)),
            )),
        )
        .map(|(name, generics)| {
            Self::Attr(Attr {
                name,
                generics: generics.unwrap_or_default(),
            })
        })
        .parse_next(i)
    }

    fn index(i: &mut &'a str, level: Level<'_>) -> ParseResult<'a, Self> {
        preceded(
            ws('['),
            cut_err(terminated(
                ws(move |i: &mut _| Expr::parse(i, level, true)),
                ']',
            )),
        )
        .map(Self::Index)
        .parse_next(i)
    }

    fn call(i: &mut &'a str, level: Level<'_>) -> ParseResult<'a, Self> {
        (opt(|i: &mut _| call_generics(i, level)), |i: &mut _| {
            Expr::arguments(i, level, false)
        })
            .map(|(generics, args)| Self::Call {
                args,
                generics: generics.unwrap_or_default(),
            })
            .parse_next(i)
    }

    fn r#try(i: &mut &'a str) -> ParseResult<'a, Self> {
        preceded(skip_ws0, '?').map(|_| Self::Try).parse_next(i)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TyGenerics<'a> {
    pub refs: usize,
    pub path: Vec<&'a str>,
    pub args: Vec<WithSpan<'a, TyGenerics<'a>>>,
}

impl<'i> TyGenerics<'i> {
    fn parse(i: &mut &'i str, level: Level<'_>) -> ParseResult<'i, WithSpan<'i, Self>> {
        let start = *i;
        (
            repeat(0.., ws('&')),
            separated(1.., ws(identifier), "::"),
            opt(|i: &mut _| Self::args(i, level)).map(|generics| generics.unwrap_or_default()),
        )
            .map(|(refs, path, args)| WithSpan::new(TyGenerics { refs, path, args }, start))
            .parse_next(i)
    }

    fn args(
        i: &mut &'i str,
        level: Level<'_>,
    ) -> ParseResult<'i, Vec<WithSpan<'i, TyGenerics<'i>>>> {
        ws('<').parse_next(i)?;
        let _level_guard = level.nest(i)?;
        cut_err(terminated(
            terminated(
                separated(0.., |i: &mut _| TyGenerics::parse(i, level), ws(',')),
                ws(opt(',')),
            ),
            '>',
        ))
        .parse_next(i)
    }
}

pub(crate) fn call_generics<'i>(
    i: &mut &'i str,
    level: Level<'_>,
) -> ParseResult<'i, Vec<WithSpan<'i, TyGenerics<'i>>>> {
    preceded(ws("::"), cut_err(|i: &mut _| TyGenerics::args(i, level))).parse_next(i)
}
