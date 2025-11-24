use crate::error::Result;
use crate::segment::{self, Segment};
use proc_macro::{Delimiter, Group, Spacing, Span, TokenStream, TokenTree};
use std::iter;
use std::mem;
use std::str::FromStr;

pub fn expand_attr(
    attr: TokenStream,
    span: Span,
    contains_paste: &mut bool,
) -> Result<TokenStream> {
    let mut tokens = attr.clone().into_iter();
    let mut leading_colons = 0; // $(::)?
    let mut leading_path = 0; // $($ident)::+

    let mut token;
    let group = loop {
        token = tokens.next();
        match token {
            // colon after `$(:)?`
            Some(TokenTree::Punct(ref punct))
                if punct.as_char() == ':' && leading_colons < 2 && leading_path == 0 =>
            {
                leading_colons += 1;
            }
            // ident after `$(::)? $($ident ::)*`
            Some(TokenTree::Ident(_)) if leading_colons != 1 && leading_path % 3 == 0 => {
                leading_path += 1;
            }
            // colon after `$(::)? $($ident ::)* $ident $(:)?`
            Some(TokenTree::Punct(ref punct)) if punct.as_char() == ':' && leading_path % 3 > 0 => {
                leading_path += 1;
            }
            // eq+value after `$(::)? $($ident)::+`
            Some(TokenTree::Punct(ref punct))
                if punct.as_char() == '=' && leading_path % 3 == 1 =>
            {
                let mut count = 0;
                if tokens.inspect(|_| count += 1).all(|tt| is_stringlike(&tt)) && count > 1 {
                    *contains_paste = true;
                    let leading = leading_colons + leading_path;
                    return do_paste_name_value_attr(attr, span, leading);
                }
                return Ok(attr);
            }
            // parens after `$(::)? $($ident)::+`
            Some(TokenTree::Group(ref group))
                if group.delimiter() == Delimiter::Parenthesis && leading_path % 3 == 1 =>
            {
                break group;
            }
            // bail out
            _ => return Ok(attr),
        }
    };

    // There can't be anything else after the first group in a valid attribute.
    if tokens.next().is_some() {
        return Ok(attr);
    }

    let mut group_contains_paste = false;
    let mut expanded = TokenStream::new();
    let mut nested_attr = TokenStream::new();
    for tt in group.stream() {
        match &tt {
            TokenTree::Punct(punct) if punct.as_char() == ',' => {
                expanded.extend(expand_attr(
                    nested_attr,
                    group.span(),
                    &mut group_contains_paste,
                )?);
                expanded.extend(iter::once(tt));
                nested_attr = TokenStream::new();
            }
            _ => nested_attr.extend(iter::once(tt)),
        }
    }

    if !nested_attr.is_empty() {
        expanded.extend(expand_attr(
            nested_attr,
            group.span(),
            &mut group_contains_paste,
        )?);
    }

    if group_contains_paste {
        *contains_paste = true;
        let mut group = Group::new(Delimiter::Parenthesis, expanded);
        group.set_span(span);
        Ok(attr
            .into_iter()
            // Just keep the initial ident in `#[ident(...)]`.
            .take(leading_colons + leading_path)
            .chain(iter::once(TokenTree::Group(group)))
            .collect())
    } else {
        Ok(attr)
    }
}

fn do_paste_name_value_attr(attr: TokenStream, span: Span, leading: usize) -> Result<TokenStream> {
    let mut expanded = TokenStream::new();
    let mut tokens = attr.into_iter().peekable();
    expanded.extend(tokens.by_ref().take(leading + 1)); // `doc =`

    let mut segments = segment::parse(&mut tokens)?;

    for segment in &mut segments {
        if let Segment::String(string) = segment {
            if let Some(open_quote) = string.value.find('"') {
                if open_quote == 0 {
                    string.value.truncate(string.value.len() - 1);
                    string.value.remove(0);
                } else {
                    let begin = open_quote + 1;
                    let end = string.value.rfind('"').unwrap();
                    let raw_string = mem::replace(&mut string.value, String::new());
                    for ch in raw_string[begin..end].chars() {
                        string.value.extend(ch.escape_default());
                    }
                }
            }
        }
    }

    let mut lit = segment::paste(&segments)?;
    lit.insert(0, '"');
    lit.push('"');

    let mut lit = TokenStream::from_str(&lit)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    lit.set_span(span);
    expanded.extend(iter::once(lit));
    Ok(expanded)
}

fn is_stringlike(token: &TokenTree) -> bool {
    match token {
        TokenTree::Ident(_) => true,
        TokenTree::Literal(literal) => {
            let repr = literal.to_string();
            !repr.starts_with('b') && !repr.starts_with('\'')
        }
        TokenTree::Group(group) => {
            if group.delimiter() != Delimiter::None {
                return false;
            }
            let mut inner = group.stream().into_iter();
            match inner.next() {
                Some(first) => inner.next().is_none() && is_stringlike(&first),
                None => false,
            }
        }
        TokenTree::Punct(punct) => {
            punct.as_char() == '\'' || punct.as_char() == ':' && punct.spacing() == Spacing::Alone
        }
    }
}
