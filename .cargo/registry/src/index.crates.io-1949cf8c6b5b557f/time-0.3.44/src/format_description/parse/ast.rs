//! AST for parsing format descriptions.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::iter;

use super::{lexer, unused, Error, Location, Spanned, SpannedValue, Unused};
use crate::internal_macros::bug;

/// One part of a complete format description.
pub(super) enum Item<'a> {
    /// A literal string, formatted and parsed as-is.
    ///
    /// This should never be present inside a nested format description.
    Literal(Spanned<&'a [u8]>),
    /// A sequence of brackets. The first acts as the escape character.
    ///
    /// This should never be present if the lexer has `BACKSLASH_ESCAPE` set to `true`.
    EscapedBracket {
        /// The first bracket.
        _first: Unused<Location>,
        /// The second bracket.
        _second: Unused<Location>,
    },
    /// Part of a type, along with its modifiers.
    Component {
        /// Where the opening bracket was in the format string.
        _opening_bracket: Unused<Location>,
        /// Whitespace between the opening bracket and name.
        _leading_whitespace: Unused<Option<Spanned<&'a [u8]>>>,
        /// The name of the component.
        name: Spanned<&'a [u8]>,
        /// The modifiers for the component.
        modifiers: Box<[Modifier<'a>]>,
        /// Whitespace between the modifiers and closing bracket.
        _trailing_whitespace: Unused<Option<Spanned<&'a [u8]>>>,
        /// Where the closing bracket was in the format string.
        _closing_bracket: Unused<Location>,
    },
    /// An optional sequence of items.
    Optional {
        /// Where the opening bracket was in the format string.
        opening_bracket: Location,
        /// Whitespace between the opening bracket and "optional".
        _leading_whitespace: Unused<Option<Spanned<&'a [u8]>>>,
        /// The "optional" keyword.
        _optional_kw: Unused<Spanned<&'a [u8]>>,
        /// Whitespace between the "optional" keyword and the opening bracket.
        _whitespace: Unused<Spanned<&'a [u8]>>,
        /// The items within the optional sequence.
        nested_format_description: NestedFormatDescription<'a>,
        /// Where the closing bracket was in the format string.
        closing_bracket: Location,
    },
    /// The first matching parse of a sequence of items.
    First {
        /// Where the opening bracket was in the format string.
        opening_bracket: Location,
        /// Whitespace between the opening bracket and "first".
        _leading_whitespace: Unused<Option<Spanned<&'a [u8]>>>,
        /// The "first" keyword.
        _first_kw: Unused<Spanned<&'a [u8]>>,
        /// Whitespace between the "first" keyword and the opening bracket.
        _whitespace: Unused<Spanned<&'a [u8]>>,
        /// The sequences of items to try.
        nested_format_descriptions: Box<[NestedFormatDescription<'a>]>,
        /// Where the closing bracket was in the format string.
        closing_bracket: Location,
    },
}

/// A format description that is nested within another format description.
pub(super) struct NestedFormatDescription<'a> {
    /// Where the opening bracket was in the format string.
    pub(super) _opening_bracket: Unused<Location>,
    /// The items within the nested format description.
    pub(super) items: Box<[Item<'a>]>,
    /// Where the closing bracket was in the format string.
    pub(super) _closing_bracket: Unused<Location>,
    /// Whitespace between the closing bracket and the next item.
    pub(super) _trailing_whitespace: Unused<Option<Spanned<&'a [u8]>>>,
}

/// A modifier for a component.
pub(super) struct Modifier<'a> {
    /// Whitespace preceding the modifier.
    pub(super) _leading_whitespace: Unused<Spanned<&'a [u8]>>,
    /// The key of the modifier.
    pub(super) key: Spanned<&'a [u8]>,
    /// Where the colon of the modifier was in the format string.
    pub(super) _colon: Unused<Location>,
    /// The value of the modifier.
    pub(super) value: Spanned<&'a [u8]>,
}

/// Parse the provided tokens into an AST.
#[inline]
pub(super) fn parse<
    'item: 'iter,
    'iter,
    I: Iterator<Item = Result<lexer::Token<'item>, Error>>,
    const VERSION: usize,
>(
    tokens: &'iter mut lexer::Lexed<I>,
) -> impl Iterator<Item = Result<Item<'item>, Error>> + 'iter {
    validate_version!(VERSION);
    parse_inner::<_, false, VERSION>(tokens)
}

/// Parse the provided tokens into an AST. The const generic indicates whether the resulting
/// [`Item`] will be used directly or as part of a [`NestedFormatDescription`].
#[inline]
fn parse_inner<
    'item,
    I: Iterator<Item = Result<lexer::Token<'item>, Error>>,
    const NESTED: bool,
    const VERSION: usize,
>(
    tokens: &mut lexer::Lexed<I>,
) -> impl Iterator<Item = Result<Item<'item>, Error>> + '_ {
    validate_version!(VERSION);
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
            lexer::Token::ComponentPart {
                kind: _, // whitespace is significant in nested components
                value,
            } if NESTED => Ok(Item::Literal(value)),
            lexer::Token::ComponentPart { kind: _, value: _ } => {
                bug!("component part should have been consumed by `parse_component`")
            }
        })
    })
}

/// Parse a component. This assumes that the opening bracket has already been consumed.
fn parse_component<
    'a,
    I: Iterator<Item = Result<lexer::Token<'a>, Error>>,
    const VERSION: usize,
>(
    opening_bracket: Location,
    tokens: &mut lexer::Lexed<I>,
) -> Result<Item<'a>, Error> {
    validate_version!(VERSION);
    let leading_whitespace = tokens.next_if_whitespace();

    let Some(name) = tokens.next_if_not_whitespace() else {
        let span = match leading_whitespace {
            Some(Spanned { value: _, span }) => span,
            None => opening_bracket.to_self(),
        };
        return Err(Error {
            _inner: unused(span.error("expected component name")),
            public: crate::error::InvalidFormatDescription::MissingComponentName {
                index: span.start.byte as usize,
            },
        });
    };

    if *name == b"optional" {
        let Some(whitespace) = tokens.next_if_whitespace() else {
            return Err(Error {
                _inner: unused(name.span.error("expected whitespace after `optional`")),
                public: crate::error::InvalidFormatDescription::Expected {
                    what: "whitespace after `optional`",
                    index: name.span.end.byte as usize,
                },
            });
        };

        let nested = parse_nested::<_, VERSION>(whitespace.span.end, tokens)?;

        let Some(closing_bracket) = tokens.next_if_closing_bracket() else {
            return Err(Error {
                _inner: unused(opening_bracket.error("unclosed bracket")),
                public: crate::error::InvalidFormatDescription::UnclosedOpeningBracket {
                    index: opening_bracket.byte as usize,
                },
            });
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
            return Err(Error {
                _inner: unused(name.span.error("expected whitespace after `first`")),
                public: crate::error::InvalidFormatDescription::Expected {
                    what: "whitespace after `first`",
                    index: name.span.end.byte as usize,
                },
            });
        };

        let mut nested_format_descriptions = Vec::new();
        while let Ok(description) = parse_nested::<_, VERSION>(whitespace.span.end, tokens) {
            nested_format_descriptions.push(description);
        }

        let Some(closing_bracket) = tokens.next_if_closing_bracket() else {
            return Err(Error {
                _inner: unused(opening_bracket.error("unclosed bracket")),
                public: crate::error::InvalidFormatDescription::UnclosedOpeningBracket {
                    index: opening_bracket.byte as usize,
                },
            });
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

        // This is not necessary for proper parsing, but provides a much better error when a nested
        // description is used where it's not allowed.
        if let Some(location) = tokens.next_if_opening_bracket() {
            return Err(Error {
                _inner: unused(
                    location
                        .to_self()
                        .error("modifier must be of the form `key:value`"),
                ),
                public: crate::error::InvalidFormatDescription::InvalidModifier {
                    value: String::from("["),
                    index: location.byte as usize,
                },
            });
        }

        let Some(Spanned { value, span }) = tokens.next_if_not_whitespace() else {
            break Some(whitespace);
        };

        let Some(colon_index) = value.iter().position(|&b| b == b':') else {
            return Err(Error {
                _inner: unused(span.error("modifier must be of the form `key:value`")),
                public: crate::error::InvalidFormatDescription::InvalidModifier {
                    value: String::from_utf8_lossy(value).into_owned(),
                    index: span.start.byte as usize,
                },
            });
        };
        let key = &value[..colon_index];
        let value = &value[colon_index + 1..];

        if key.is_empty() {
            return Err(Error {
                _inner: unused(span.shrink_to_start().error("expected modifier key")),
                public: crate::error::InvalidFormatDescription::InvalidModifier {
                    value: String::new(),
                    index: span.start.byte as usize,
                },
            });
        }
        if value.is_empty() {
            return Err(Error {
                _inner: unused(span.shrink_to_end().error("expected modifier value")),
                public: crate::error::InvalidFormatDescription::InvalidModifier {
                    value: String::new(),
                    index: span.shrink_to_end().start.byte as usize,
                },
            });
        }

        modifiers.push(Modifier {
            _leading_whitespace: unused(whitespace),
            key: key.spanned(span.shrink_to_before(colon_index as u32)),
            _colon: unused(span.start.offset(colon_index as u32)),
            value: value.spanned(span.shrink_to_after(colon_index as u32)),
        });
    };

    let Some(closing_bracket) = tokens.next_if_closing_bracket() else {
        return Err(Error {
            _inner: unused(opening_bracket.error("unclosed bracket")),
            public: crate::error::InvalidFormatDescription::UnclosedOpeningBracket {
                index: opening_bracket.byte as usize,
            },
        });
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

/// Parse a nested format description. The location provided is the most recent one consumed.
#[inline]
fn parse_nested<'a, I: Iterator<Item = Result<lexer::Token<'a>, Error>>, const VERSION: usize>(
    last_location: Location,
    tokens: &mut lexer::Lexed<I>,
) -> Result<NestedFormatDescription<'a>, Error> {
    validate_version!(VERSION);
    let Some(opening_bracket) = tokens.next_if_opening_bracket() else {
        return Err(Error {
            _inner: unused(last_location.error("expected opening bracket")),
            public: crate::error::InvalidFormatDescription::Expected {
                what: "opening bracket",
                index: last_location.byte as usize,
            },
        });
    };
    let items = parse_inner::<_, true, VERSION>(tokens).collect::<Result<_, _>>()?;
    let Some(closing_bracket) = tokens.next_if_closing_bracket() else {
        return Err(Error {
            _inner: unused(opening_bracket.error("unclosed bracket")),
            public: crate::error::InvalidFormatDescription::UnclosedOpeningBracket {
                index: opening_bracket.byte as usize,
            },
        });
    };
    let trailing_whitespace = tokens.next_if_whitespace();

    Ok(NestedFormatDescription {
        _opening_bracket: unused(opening_bracket),
        items,
        _closing_bracket: unused(closing_bracket),
        _trailing_whitespace: unused(trailing_whitespace),
    })
}
