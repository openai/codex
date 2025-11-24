use crate::error::{Error, Result};
use proc_macro::{token_stream, Delimiter, Ident, Span, TokenTree};
use std::iter::Peekable;

pub(crate) enum Segment {
    String(LitStr),
    Apostrophe(Span),
    Env(LitStr),
    Modifier(Colon, Ident),
}

pub(crate) struct LitStr {
    pub value: String,
    pub span: Span,
}

pub(crate) struct Colon {
    pub span: Span,
}

pub(crate) fn parse(tokens: &mut Peekable<token_stream::IntoIter>) -> Result<Vec<Segment>> {
    let mut segments = Vec::new();
    while match tokens.peek() {
        None => false,
        Some(TokenTree::Punct(punct)) => punct.as_char() != '>',
        Some(_) => true,
    } {
        match tokens.next().unwrap() {
            TokenTree::Ident(ident) => {
                let mut fragment = ident.to_string();
                if fragment.starts_with("r#") {
                    fragment = fragment.split_off(2);
                }
                if fragment == "env"
                    && match tokens.peek() {
                        Some(TokenTree::Punct(punct)) => punct.as_char() == '!',
                        _ => false,
                    }
                {
                    let bang = tokens.next().unwrap(); // `!`
                    let expect_group = tokens.next();
                    let parenthesized = match &expect_group {
                        Some(TokenTree::Group(group))
                            if group.delimiter() == Delimiter::Parenthesis =>
                        {
                            group
                        }
                        Some(wrong) => return Err(Error::new(wrong.span(), "expected `(`")),
                        None => {
                            return Err(Error::new2(
                                ident.span(),
                                bang.span(),
                                "expected `(` after `env!`",
                            ));
                        }
                    };
                    let mut inner = parenthesized.stream().into_iter();
                    let lit = match inner.next() {
                        Some(TokenTree::Literal(lit)) => lit,
                        Some(wrong) => {
                            return Err(Error::new(wrong.span(), "expected string literal"))
                        }
                        None => {
                            return Err(Error::new2(
                                ident.span(),
                                parenthesized.span(),
                                "expected string literal as argument to env! macro",
                            ))
                        }
                    };
                    let lit_string = lit.to_string();
                    if lit_string.starts_with('"')
                        && lit_string.ends_with('"')
                        && lit_string.len() >= 2
                    {
                        // TODO: maybe handle escape sequences in the string if
                        // someone has a use case.
                        segments.push(Segment::Env(LitStr {
                            value: lit_string[1..lit_string.len() - 1].to_owned(),
                            span: lit.span(),
                        }));
                    } else {
                        return Err(Error::new(lit.span(), "expected string literal"));
                    }
                    if let Some(unexpected) = inner.next() {
                        return Err(Error::new(
                            unexpected.span(),
                            "unexpected token in env! macro",
                        ));
                    }
                } else {
                    segments.push(Segment::String(LitStr {
                        value: fragment,
                        span: ident.span(),
                    }));
                }
            }
            TokenTree::Literal(lit) => {
                segments.push(Segment::String(LitStr {
                    value: lit.to_string(),
                    span: lit.span(),
                }));
            }
            TokenTree::Punct(punct) => match punct.as_char() {
                '_' => segments.push(Segment::String(LitStr {
                    value: "_".to_owned(),
                    span: punct.span(),
                })),
                '\'' => segments.push(Segment::Apostrophe(punct.span())),
                ':' => {
                    let colon_span = punct.span();
                    let colon = Colon { span: colon_span };
                    let ident = match tokens.next() {
                        Some(TokenTree::Ident(ident)) => ident,
                        wrong => {
                            let span = wrong.as_ref().map_or(colon_span, TokenTree::span);
                            return Err(Error::new(span, "expected identifier after `:`"));
                        }
                    };
                    segments.push(Segment::Modifier(colon, ident));
                }
                _ => return Err(Error::new(punct.span(), "unexpected punct")),
            },
            TokenTree::Group(group) => {
                if group.delimiter() == Delimiter::None {
                    let mut inner = group.stream().into_iter().peekable();
                    let nested = parse(&mut inner)?;
                    if let Some(unexpected) = inner.next() {
                        return Err(Error::new(unexpected.span(), "unexpected token"));
                    }
                    segments.extend(nested);
                } else {
                    return Err(Error::new(group.span(), "unexpected token"));
                }
            }
        }
    }
    Ok(segments)
}

pub(crate) fn paste(segments: &[Segment]) -> Result<String> {
    let mut evaluated = Vec::new();
    let mut is_lifetime = false;

    for segment in segments {
        match segment {
            Segment::String(segment) => {
                evaluated.push(segment.value.clone());
            }
            Segment::Apostrophe(span) => {
                if is_lifetime {
                    return Err(Error::new(*span, "unexpected lifetime"));
                }
                is_lifetime = true;
            }
            Segment::Env(var) => {
                let resolved = match std::env::var(&var.value) {
                    Ok(resolved) => resolved,
                    Err(_) => {
                        return Err(Error::new(
                            var.span,
                            &format!("no such env var: {:?}", var.value),
                        ));
                    }
                };
                let resolved = resolved.replace('-', "_");
                evaluated.push(resolved);
            }
            Segment::Modifier(colon, ident) => {
                let last = match evaluated.pop() {
                    Some(last) => last,
                    None => {
                        return Err(Error::new2(colon.span, ident.span(), "unexpected modifier"))
                    }
                };
                match ident.to_string().as_str() {
                    "lower" => {
                        evaluated.push(last.to_lowercase());
                    }
                    "upper" => {
                        evaluated.push(last.to_uppercase());
                    }
                    "snake" => {
                        let mut acc = String::new();
                        let mut prev = '_';
                        for ch in last.chars() {
                            if ch.is_uppercase() && prev != '_' {
                                acc.push('_');
                            }
                            acc.push(ch);
                            prev = ch;
                        }
                        evaluated.push(acc.to_lowercase());
                    }
                    "camel" => {
                        let mut acc = String::new();
                        let mut prev = '_';
                        for ch in last.chars() {
                            if ch != '_' {
                                if prev == '_' {
                                    for chu in ch.to_uppercase() {
                                        acc.push(chu);
                                    }
                                } else if prev.is_uppercase() {
                                    for chl in ch.to_lowercase() {
                                        acc.push(chl);
                                    }
                                } else {
                                    acc.push(ch);
                                }
                            }
                            prev = ch;
                        }
                        evaluated.push(acc);
                    }
                    _ => {
                        return Err(Error::new2(
                            colon.span,
                            ident.span(),
                            "unsupported modifier",
                        ));
                    }
                }
            }
        }
    }

    let mut pasted = evaluated.into_iter().collect::<String>();
    if is_lifetime {
        pasted.insert(0, '\'');
    }
    Ok(pasted)
}
