//! Lexer for parsing format descriptions.

use core::iter;

use super::{attach_location, unused, Error, Location, Spanned, SpannedValue};

/// An iterator over the lexed tokens.
pub(super) struct Lexed<I: Iterator> {
    /// The internal iterator.
    iter: iter::Peekable<I>,
}

impl<I: Iterator> Iterator for Lexed<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'iter, 'token: 'iter, I: Iterator<Item = Result<Token<'token>, Error>> + 'iter> Lexed<I> {
    /// Peek at the next item in the iterator.
    #[inline]
    pub(super) fn peek(&mut self) -> Option<&I::Item> {
        self.iter.peek()
    }

    /// Consume the next token if it is whitespace.
    #[inline]
    pub(super) fn next_if_whitespace(&mut self) -> Option<Spanned<&'token [u8]>> {
        if let Some(&Ok(Token::ComponentPart {
            kind: ComponentKind::Whitespace,
            value,
        })) = self.peek()
        {
            self.next(); // consume
            Some(value)
        } else {
            None
        }
    }

    /// Consume the next token if it is a component item that is not whitespace.
    #[inline]
    pub(super) fn next_if_not_whitespace(&mut self) -> Option<Spanned<&'token [u8]>> {
        if let Some(&Ok(Token::ComponentPart {
            kind: ComponentKind::NotWhitespace,
            value,
        })) = self.peek()
        {
            self.next(); // consume
            Some(value)
        } else {
            None
        }
    }

    /// Consume the next token if it is an opening bracket.
    #[inline]
    pub(super) fn next_if_opening_bracket(&mut self) -> Option<Location> {
        if let Some(&Ok(Token::Bracket {
            kind: BracketKind::Opening,
            location,
        })) = self.peek()
        {
            self.next(); // consume
            Some(location)
        } else {
            None
        }
    }

    /// Peek at the next token if it is a closing bracket.
    #[inline]
    pub(super) fn peek_closing_bracket(&'iter mut self) -> Option<&'iter Location> {
        if let Some(Ok(Token::Bracket {
            kind: BracketKind::Closing,
            location,
        })) = self.peek()
        {
            Some(location)
        } else {
            None
        }
    }

    /// Consume the next token if it is a closing bracket.
    #[inline]
    pub(super) fn next_if_closing_bracket(&mut self) -> Option<Location> {
        if let Some(&Ok(Token::Bracket {
            kind: BracketKind::Closing,
            location,
        })) = self.peek()
        {
            self.next(); // consume
            Some(location)
        } else {
            None
        }
    }
}

/// A token emitted by the lexer. There is no semantic meaning at this stage.
pub(super) enum Token<'a> {
    /// A literal string, formatted and parsed as-is.
    Literal(Spanned<&'a [u8]>),
    /// An opening or closing bracket. May or may not be the start or end of a component.
    Bracket {
        /// Whether the bracket is opening or closing.
        kind: BracketKind,
        /// Where the bracket was in the format string.
        location: Location,
    },
    /// One part of a component. This could be its name, a modifier, or whitespace.
    ComponentPart {
        /// Whether the part is whitespace or not.
        kind: ComponentKind,
        /// The part itself.
        value: Spanned<&'a [u8]>,
    },
}

/// What type of bracket is present.
pub(super) enum BracketKind {
    /// An opening bracket: `[`
    Opening,
    /// A closing bracket: `]`
    Closing,
}

/// Indicates whether the component is whitespace or not.
pub(super) enum ComponentKind {
    Whitespace,
    NotWhitespace,
}

/// Parse the string into a series of [`Token`]s.
///
/// `VERSION` controls the version of the format description that is being parsed. Currently, this
/// must be 1 or 2.
///
/// - When `VERSION` is 1, `[[` is the only escape sequence, resulting in a literal `[`.
/// - When `VERSION` is 2, all escape sequences begin with `\`. The only characters that may
///   currently follow are `\`, `[`, and `]`, all of which result in the literal character. All
///   other characters result in a lex error.
#[inline]
pub(super) fn lex<const VERSION: usize>(
    mut input: &[u8],
) -> Lexed<impl Iterator<Item = Result<Token<'_>, Error>>> {
    validate_version!(VERSION);

    let mut depth: u8 = 0;
    let mut iter = attach_location(input.iter()).peekable();
    let mut second_bracket_location = None;

    let iter = iter::from_fn(move || {
        // The flag is only set when version is zero.
        if version!(..=1) {
            // There is a flag set to emit the second half of an escaped bracket pair.
            if let Some(location) = second_bracket_location.take() {
                return Some(Ok(Token::Bracket {
                    kind: BracketKind::Opening,
                    location,
                }));
            }
        }

        Some(Ok(match iter.next()? {
            // possible escape sequence
            (b'\\', backslash_loc) if version!(2..) => {
                match iter.next() {
                    Some((b'\\' | b'[' | b']', char_loc)) => {
                        // The escaped character is emitted as-is.
                        let char = &input[1..2];
                        input = &input[2..];
                        if depth == 0 {
                            Token::Literal(char.spanned(backslash_loc.to(char_loc)))
                        } else {
                            Token::ComponentPart {
                                kind: ComponentKind::NotWhitespace,
                                value: char.spanned(backslash_loc.to(char_loc)),
                            }
                        }
                    }
                    Some((_, loc)) => {
                        return Some(Err(Error {
                            _inner: unused(loc.error("invalid escape sequence")),
                            public: crate::error::InvalidFormatDescription::Expected {
                                what: "valid escape sequence",
                                index: loc.byte as usize,
                            },
                        }));
                    }
                    None => {
                        return Some(Err(Error {
                            _inner: unused(backslash_loc.error("unexpected end of input")),
                            public: crate::error::InvalidFormatDescription::Expected {
                                what: "valid escape sequence",
                                index: backslash_loc.byte as usize,
                            },
                        }));
                    }
                }
            }
            // potentially escaped opening bracket
            (b'[', location) if version!(..=1) => {
                if let Some((_, second_location)) = iter.next_if(|&(&byte, _)| byte == b'[') {
                    // Escaped bracket. Store the location of the second so we can emit it later.
                    second_bracket_location = Some(second_location);
                    input = &input[2..];
                } else {
                    // opening bracket
                    depth += 1;
                    input = &input[1..];
                }

                Token::Bracket {
                    kind: BracketKind::Opening,
                    location,
                }
            }
            // opening bracket
            (b'[', location) => {
                depth += 1;
                input = &input[1..];

                Token::Bracket {
                    kind: BracketKind::Opening,
                    location,
                }
            }
            // closing bracket
            (b']', location) if depth > 0 => {
                depth -= 1;
                input = &input[1..];

                Token::Bracket {
                    kind: BracketKind::Closing,
                    location,
                }
            }
            // literal
            (_, start_location) if depth == 0 => {
                let mut bytes = 1;
                let mut end_location = start_location;

                while let Some((_, location)) =
                    iter.next_if(|&(&byte, _)| !((version!(2..) && byte == b'\\') || byte == b'['))
                {
                    end_location = location;
                    bytes += 1;
                }

                let value = &input[..bytes];
                input = &input[bytes..];

                Token::Literal(value.spanned(start_location.to(end_location)))
            }
            // component part
            (byte, start_location) => {
                let mut bytes = 1;
                let mut end_location = start_location;
                let is_whitespace = byte.is_ascii_whitespace();

                while let Some((_, location)) = iter.next_if(|&(byte, _)| {
                    !matches!(byte, b'\\' | b'[' | b']')
                        && is_whitespace == byte.is_ascii_whitespace()
                }) {
                    end_location = location;
                    bytes += 1;
                }

                let value = &input[..bytes];
                input = &input[bytes..];

                Token::ComponentPart {
                    kind: if is_whitespace {
                        ComponentKind::Whitespace
                    } else {
                        ComponentKind::NotWhitespace
                    },
                    value: value.spanned(start_location.to(end_location)),
                }
            }
        }))
    });

    Lexed {
        iter: iter.peekable(),
    }
}
