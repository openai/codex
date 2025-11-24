use winnow::combinator::{alt, opt, peek, preceded, separated};
use winnow::token::one_of;
use winnow::{ModalParser, Parser};

use crate::{
    CharLit, ErrorContext, Num, ParseErr, ParseResult, PathOrIdentifier, State, StrLit, WithSpan,
    bool_lit, char_lit, identifier, is_rust_keyword, keyword, num_lit, path_or_identifier, str_lit,
    ws,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Target<'a> {
    Name(&'a str),
    Tuple(Vec<&'a str>, Vec<Target<'a>>),
    Array(Vec<&'a str>, Vec<Target<'a>>),
    Struct(Vec<&'a str>, Vec<(&'a str, Target<'a>)>),
    NumLit(&'a str, Num<'a>),
    StrLit(StrLit<'a>),
    CharLit(CharLit<'a>),
    BoolLit(&'a str),
    Path(Vec<&'a str>),
    OrChain(Vec<Target<'a>>),
    Placeholder(WithSpan<'a, ()>),
    /// The `Option` is the variable name (if any) in `var_name @ ..`.
    Rest(WithSpan<'a, Option<&'a str>>),
}

impl<'a> Target<'a> {
    /// Parses multiple targets with `or` separating them
    pub(super) fn parse(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Self> {
        let _level_guard = s.level.nest(i)?;
        let mut p = opt(preceded(ws(keyword("or")), |i: &mut _| {
            Self::parse_one(i, s)
        }));

        let target = Self::parse_one(i, s)?;
        let Some(snd_target) = p.parse_next(i)? else {
            return Ok(target);
        };

        let mut targets = vec![target, snd_target];
        while let Some(target) = p.parse_next(i)? {
            targets.push(target);
        }
        Ok(Self::OrChain(targets))
    }

    /// Parses a single target without an `or`, unless it is wrapped in parentheses.
    fn parse_one(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Self> {
        let mut opt_opening_paren = opt(ws('(')).map(|o| o.is_some());
        let mut opt_opening_brace = opt(ws('{')).map(|o| o.is_some());
        let mut opt_opening_bracket = opt(ws('[')).map(|o| o.is_some());

        let lit = opt(Self::lit).parse_next(i)?;
        if let Some(lit) = lit {
            return Ok(lit);
        }

        // match tuples and unused parentheses
        let target_is_tuple = opt_opening_paren.parse_next(i)?;
        if target_is_tuple {
            let (singleton, mut targets) =
                collect_targets(i, ')', |i: &mut _| Self::unnamed(i, s))?;
            if singleton {
                return Ok(targets.pop().unwrap());
            }
            return Ok(Self::Tuple(
                Vec::new(),
                only_one_rest_pattern(targets, false, "tuple")?,
            ));
        }
        let target_is_array = opt_opening_bracket.parse_next(i)?;
        if target_is_array {
            let (singleton, mut targets) =
                collect_targets(i, ']', |i: &mut _| Self::unnamed(i, s))?;
            if singleton {
                return Ok(targets.pop().unwrap());
            }
            return Ok(Self::Array(
                Vec::new(),
                only_one_rest_pattern(targets, true, "array")?,
            ));
        }

        let path = path_or_identifier.try_map(|v| match v {
            PathOrIdentifier::Path(v) => Ok(v),
            PathOrIdentifier::Identifier(v) => Err(v),
        });

        // match structs
        let path = opt(path).parse_next(i)?;
        if let Some(path) = path {
            let i_before_matching_with = *i;
            let _ = opt(ws(keyword("with"))).parse_next(i)?;

            let is_unnamed_struct = opt_opening_paren.parse_next(i)?;
            if is_unnamed_struct {
                let (_, targets) = collect_targets(i, ')', |i: &mut _| Self::unnamed(i, s))?;
                return Ok(Self::Tuple(
                    path,
                    only_one_rest_pattern(targets, false, "struct")?,
                ));
            }

            let is_named_struct = opt_opening_brace.parse_next(i)?;
            if is_named_struct {
                let (_, targets) = collect_targets(i, '}', |i: &mut _| Self::named(i, s))?;
                return Ok(Self::Struct(path, targets));
            }

            *i = i_before_matching_with;
            return Ok(Self::Path(path));
        }

        // neither literal nor struct nor path
        let i_before_identifier = *i;
        let name = identifier.parse_next(i)?;
        let target = match name {
            "_" => Self::Placeholder(WithSpan::new((), i_before_identifier)),
            _ => verify_name(i_before_identifier, name)?,
        };
        Ok(target)
    }

    fn lit(i: &mut &'a str) -> ParseResult<'a, Self> {
        alt((
            str_lit.map(Self::StrLit),
            char_lit.map(Self::CharLit),
            num_lit
                .with_taken()
                .map(|(num, full)| Target::NumLit(full, num)),
            bool_lit.map(Self::BoolLit),
        ))
        .parse_next(i)
    }

    fn unnamed(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, Self> {
        alt((Self::rest, |i: &mut _| Self::parse(i, s))).parse_next(i)
    }

    fn named(i: &mut &'a str, s: &State<'_, '_>) -> ParseResult<'a, (&'a str, Self)> {
        let start = *i;
        let rest = opt(Self::rest.with_taken()).parse_next(i)?;
        if let Some(rest) = rest {
            let chr = peek(ws(opt(one_of([',', ':'])))).parse_next(i)?;
            if let Some(chr) = chr {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    format!(
                        "unexpected `{chr}` character after `..`\n\
                         note that in a named struct, `..` must come last to ignore other members"
                    ),
                    *i,
                )));
            }
            if let Target::Rest(ref s) = rest.0 {
                if s.inner.is_some() {
                    return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                        "`@ ..` cannot be used in struct",
                        s.span,
                    )));
                }
            }
            return Ok((rest.1, rest.0));
        }

        *i = start;
        let (src, target) = (
            identifier,
            opt(preceded(ws(':'), |i: &mut _| Self::parse(i, s))),
        )
            .parse_next(i)?;

        if src == "_" {
            *i = start;
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                "cannot use placeholder `_` as source in named struct",
                *i,
            )));
        }

        let target = match target {
            Some(target) => target,
            None => verify_name(start, src)?,
        };
        Ok((src, target))
    }

    fn rest(i: &mut &'a str) -> ParseResult<'a, Self> {
        let start = *i;
        let (ident, _) = (opt((identifier, ws('@'))), "..").parse_next(i)?;
        Ok(Self::Rest(WithSpan::new(
            ident.map(|(ident, _)| ident),
            start,
        )))
    }
}

fn verify_name<'a>(
    input: &'a str,
    name: &'a str,
) -> Result<Target<'a>, winnow::error::ErrMode<ErrorContext<'a>>> {
    if is_rust_keyword(name) {
        Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            format!("cannot use `{name}` as a name: it is a rust keyword"),
            input,
        )))
    } else if name.starts_with("__askama") {
        Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            format!("cannot use `{name}` as a name: it is reserved for `askama`"),
            input,
        )))
    } else {
        Ok(Target::Name(name))
    }
}

fn collect_targets<'a, T>(
    i: &mut &'a str,
    delim: char,
    one: impl ModalParser<&'a str, T, ErrorContext<'a>>,
) -> ParseResult<'a, (bool, Vec<T>)> {
    let opt_comma = ws(opt(',')).map(|o| o.is_some());
    let mut opt_end = ws(opt(one_of(delim))).map(|o| o.is_some());

    let has_end = opt_end.parse_next(i)?;
    if has_end {
        return Ok((false, Vec::new()));
    }

    let targets = opt(separated(1.., one, ws(',')).map(|v: Vec<_>| v)).parse_next(i)?;
    let Some(targets) = targets else {
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "expected comma separated list of members",
            *i,
        )));
    };

    let (has_comma, has_end) = (opt_comma, opt_end).parse_next(i)?;
    if !has_end {
        let msg = match has_comma {
            true => format!("expected member, or `{delim}` as terminator"),
            false => format!("expected `,` for more members, or `{delim}` as terminator"),
        };
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(msg, *i)));
    }

    let singleton = !has_comma && targets.len() == 1;
    Ok((singleton, targets))
}

fn only_one_rest_pattern<'a>(
    targets: Vec<Target<'a>>,
    allow_named_rest: bool,
    type_kind: &str,
) -> Result<Vec<Target<'a>>, ParseErr<'a>> {
    let mut found_rest = false;

    for target in &targets {
        if let Target::Rest(s) = target {
            if !allow_named_rest && s.inner.is_some() {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    "`@ ..` is only allowed in slice patterns",
                    s.span,
                )));
            }
            if found_rest {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    format!("`..` can only be used once per {type_kind} pattern"),
                    s.span,
                )));
            }
            found_rest = true;
        }
    }
    Ok(targets)
}
