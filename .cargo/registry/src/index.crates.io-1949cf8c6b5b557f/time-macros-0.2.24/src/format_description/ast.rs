use std::iter;

use super::{lexer, unused, Error, Location, Spanned, SpannedValue, Unused};

pub(super) enum Item<'a> {
    Literal(Spanned<&'a [u8]>),
    EscapedBracket {
        _first: Unused<Location>,
        _second: Unused<Location>,
    },
    Component {
        _opening_bracket: Unused<Location>,
        _leading_whitespace: Unused<Option<Spanned<&'a [u8]>>>,
        name: Spanned<&'a [u8]>,
        modifiers: Box<[Modifier<'a>]>,
        _trailing_whitespace: Unused<Option<Spanned<&'a [u8]>>>,
        _closing_bracket: Unused<Location>,
    },
    Optional {
        opening_bracket: Location,
        _leading_whitespace: Unused<Option<Spanned<&'a [u8]>>>,
        _optional_kw: Unused<Spanned<&'a [u8]>>,
        _whitespace: Unused<Spanned<&'a [u8]>>,
        nested_format_description: NestedFormatDescription<'a>,
        closing_bracket: Location,
    },
    First {
        opening_bracket: Location,
        _leading_whitespace: Unused<Option<Spanned<&'a [u8]>>>,
        _first_kw: Unused<Spanned<&'a [u8]>>,
        _whitespace: Unused<Spanned<&'a [u8]>>,
        nested_format_descriptions: Box<[NestedFormatDescription<'a>]>,
        closing_bracket: Location,
    },
}

pub(super) struct NestedFormatDescription<'a> {
    pub(super) _opening_bracket: Unused<Location>,
    pub(super) items: Box<[Item<'a>]>,
    pub(super) _closing_bracket: Unused<Location>,
    pub(super) _trailing_whitespace: Unused<Option<Spanned<&'a [u8]>>>,
}

pub(super) struct Modifier<'a> {
    pub(super) _leading_whitespace: Unused<Spanned<&'a [u8]>>,
    pub(super) key: Spanned<&'a [u8]>,
    pub(super) _colon: Unused<Location>,
    pub(super) value: Spanned<&'a [u8]>,
}

pub(super) fn parse<
    'item: 'iter,
    'iter,
    I: Iterator<Item = Result<lexer::Token<'item>, Error>>,
    const VERSION: u8,
>(
    tokens: &'iter mut lexer::Lexed<I>,
) -> impl Iterator<Item = Result<Item<'item>, Error>> + 'iter {
    assert!(version!(1..=2));
    parse_inner::<_, false, VERSION>(tokens)
}

fn parse_inner<
    'item,
    I: Iterator<Item = Result<lexer::Token<'item>, Error>>,
    const NESTED: bool,
    const VERSION: u8,
>(
    tokens: &mut lexer::Lexed<I>,
) -> impl Iterator<Item = Result<Item<'item>, Error>> + '_ {
    iter::from_fn(move || {
        if NESTED && tokens.peek_closing_bracket().is_some() {
            return None;
        }

        let next = match tokens.next()? {
            Ok(token) => token,
            Err(err) => return Some(Err(err)),
        };

        Some(match next {
            lexer::Token::Literal(Spanned { value: _, span: _ }) if NESTED => {
                bug!("literal should not be present in nested description")
            }
            lexer::Token::Literal(value) => Ok(Item::Literal(value)),
            lexer::Token::Bracket {
                kind: lexer::BracketKind::Opening,
                location,
            } => {
                if version!(..=1) {
                    if let Some(second_location) = tokens.next_if_opening_bracket() {
                        Ok(Item::EscapedBracket {
                            _first: unused(location),
                            _second: unused(second_location),
                        })
                    } else {
                        parse_component::<_, VERSION>(location, tokens)
                    }
                } else {
                    parse_component::<_, VERSION>(location, tokens)
                }
            }
            lexer::Token::Bracket {
                kind: lexer::BracketKind::Closing,
                location: _,
            } if NESTED => {
                bug!("closing bracket should be caught by the `if` statement")
            }
            lexer::Token::Bracket {
                kind: lexer::BracketKind::Closing,
                location: _,
            } => {
                bug!("closing bracket should have been consumed by `parse_component`")
            }
            lexer::Token::ComponentPart { kind: _, value } if NESTED => Ok(Item::Literal(value)),
            lexer::Token::ComponentPart { kind: _, value: _ } => {
                bug!("component part should have been consumed by `parse_component`")
            }
        })
    })
}

fn parse_component<'a, I: Iterator<Item = Result<lexer::Token<'a>, Error>>, const VERSION: u8>(
    opening_bracket: Location,
    tokens: &mut lexer::Lexed<I>,
) -> Result<Item<'a>, Error> {
    let leading_whitespace = tokens.next_if_whitespace();

    let Some(name) = tokens.next_if_not_whitespace() else {
        let span = match leading_whitespace {
            Some(Spanned { value: _, span }) => span,
            None => opening_bracket.to(opening_bracket),
        };
        return Err(span.error("expected component name"));
    };

    if *name == b"optional" {
        let Some(whitespace) = tokens.next_if_whitespace() else {
            return Err(name.span.error("expected whitespace after `optional`"));
        };

        let nested = parse_nested::<_, VERSION>(whitespace.span.end, tokens)?;

        let Some(closing_bracket) = tokens.next_if_closing_bracket() else {
            return Err(opening_bracket.error("unclosed bracket"));
        };

        return Ok(Item::Optional {
            opening_bracket,
            _leading_whitespace: unused(leading_whitespace),
            _optional_kw: unused(name),
            _whitespace: unused(whitespace),
            nested_format_description: nested,
            closing_bracket,
        });
    }

    if *name == b"first" {
        let Some(whitespace) = tokens.next_if_whitespace() else {
            return Err(name.span.error("expected whitespace after `first`"));
        };

        let mut nested_format_descriptions = Vec::new();
        while let Ok(description) = parse_nested::<_, VERSION>(whitespace.span.end, tokens) {
            nested_format_descriptions.push(description);
        }

        let Some(closing_bracket) = tokens.next_if_closing_bracket() else {
            return Err(opening_bracket.error("unclosed bracket"));
        };

        return Ok(Item::First {
            opening_bracket,
            _leading_whitespace: unused(leading_whitespace),
            _first_kw: unused(name),
            _whitespace: unused(whitespace),
            nested_format_descriptions: nested_format_descriptions.into_boxed_slice(),
            closing_bracket,
        });
    }

    let mut modifiers = Vec::new();
    let trailing_whitespace = loop {
        let Some(whitespace) = tokens.next_if_whitespace() else {
            break None;
        };

        if let Some(location) = tokens.next_if_opening_bracket() {
            return Err(location
                .to(location)
                .error("modifier must be of the form `key:value`"));
        }

        let Some(Spanned { value, span }) = tokens.next_if_not_whitespace() else {
            break Some(whitespace);
        };

        let Some(colon_index) = value.iter().position(|&b| b == b':') else {
            return Err(span.error("modifier must be of the form `key:value`"));
        };
        let key = &value[..colon_index];
        let value = &value[colon_index + 1..];

        if key.is_empty() {
            return Err(span.shrink_to_start().error("expected modifier key"));
        }
        if value.is_empty() {
            return Err(span.shrink_to_end().error("expected modifier value"));
        }

        modifiers.push(Modifier {
            _leading_whitespace: unused(whitespace),
            key: key.spanned(span.shrink_to_before(colon_index as u32)),
            _colon: unused(span.start.offset(colon_index as u32)),
            value: value.spanned(span.shrink_to_after(colon_index as u32)),
        });
    };

    let Some(closing_bracket) = tokens.next_if_closing_bracket() else {
        return Err(opening_bracket.error("unclosed bracket"));
    };

    Ok(Item::Component {
        _opening_bracket: unused(opening_bracket),
        _leading_whitespace: unused(leading_whitespace),
        name,
        modifiers: modifiers.into_boxed_slice(),
        _trailing_whitespace: unused(trailing_whitespace),
        _closing_bracket: unused(closing_bracket),
    })
}

fn parse_nested<'a, I: Iterator<Item = Result<lexer::Token<'a>, Error>>, const VERSION: u8>(
    last_location: Location,
    tokens: &mut lexer::Lexed<I>,
) -> Result<NestedFormatDescription<'a>, Error> {
    let Some(opening_bracket) = tokens.next_if_opening_bracket() else {
        return Err(last_location.error("expected opening bracket"));
    };
    let items = parse_inner::<_, true, VERSION>(tokens).collect::<Result<_, _>>()?;
    let Some(closing_bracket) = tokens.next_if_closing_bracket() else {
        return Err(opening_bracket.error("unclosed bracket"));
    };
    let trailing_whitespace = tokens.next_if_whitespace();

    Ok(NestedFormatDescription {
        _opening_bracket: unused(opening_bracket),
        items,
        _closing_bracket: unused(closing_bracket),
        _trailing_whitespace: unused(trailing_whitespace),
    })
}
