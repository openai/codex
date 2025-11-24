use core::iter;

use super::{Error, Location, Spanned, SpannedValue};

pub(super) struct Lexed<I: Iterator> {
    iter: iter::Peekable<I>,
}

impl<I: Iterator> Iterator for Lexed<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'iter, 'token: 'iter, I: Iterator<Item = Result<Token<'token>, Error>> + 'iter> Lexed<I> {
    pub(super) fn peek(&mut self) -> Option<&I::Item> {
        self.iter.peek()
    }

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

    pub(super) fn next_if_not_whitespace(&mut self) -> Option<Spanned<&'token [u8]>> {
        if let Some(&Ok(Token::ComponentPart {
            kind: ComponentKind::NotWhitespace,
            value,
        })) = self.peek()
        {
            self.next();
            Some(value)
        } else {
            None
        }
    }

    pub(super) fn next_if_opening_bracket(&mut self) -> Option<Location> {
        if let Some(&Ok(Token::Bracket {
            kind: BracketKind::Opening,
            location,
        })) = self.peek()
        {
            self.next();
            Some(location)
        } else {
            None
        }
    }

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

    pub(super) fn next_if_closing_bracket(&mut self) -> Option<Location> {
        if let Some(&Ok(Token::Bracket {
            kind: BracketKind::Closing,
            location,
        })) = self.peek()
        {
            self.next();
            Some(location)
        } else {
            None
        }
    }
}

pub(super) enum Token<'a> {
    Literal(Spanned<&'a [u8]>),
    Bracket {
        kind: BracketKind,
        location: Location,
    },
    ComponentPart {
        kind: ComponentKind,
        value: Spanned<&'a [u8]>,
    },
}

pub(super) enum BracketKind {
    Opening,
    Closing,
}

pub(super) enum ComponentKind {
    Whitespace,
    NotWhitespace,
}

fn attach_location<'item>(
    iter: impl Iterator<Item = &'item u8>,
    proc_span: proc_macro::Span,
) -> impl Iterator<Item = (&'item u8, Location)> {
    let mut byte_pos = 0;

    iter.map(move |byte| {
        let location = Location {
            byte: byte_pos,
            proc_span,
        };
        byte_pos += 1;
        (byte, location)
    })
}

pub(super) fn lex<const VERSION: u8>(
    mut input: &[u8],
    proc_span: proc_macro::Span,
) -> Lexed<impl Iterator<Item = Result<Token<'_>, Error>>> {
    assert!(version!(1..=2));

    let mut depth: u8 = 0;
    let mut iter = attach_location(input.iter(), proc_span).peekable();
    let mut second_bracket_location = None;

    let iter = iter::from_fn(move || {
        if version!(..=1) {
            if let Some(location) = second_bracket_location.take() {
                return Some(Ok(Token::Bracket {
                    kind: BracketKind::Opening,
                    location,
                }));
            }
        }

        Some(Ok(match iter.next()? {
            (b'\\', backslash_loc) if version!(2..) => match iter.next() {
                Some((b'\\' | b'[' | b']', char_loc)) => {
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
                    return Some(Err(loc.error("invalid escape sequence")));
                }
                None => {
                    return Some(Err(backslash_loc.error("unexpected end of input")));
                }
            },
            (b'[', location) if version!(..=1) => {
                if let Some((_, second_location)) = iter.next_if(|&(&byte, _)| byte == b'[') {
                    second_bracket_location = Some(second_location);
                    input = &input[2..];
                } else {
                    depth += 1;
                    input = &input[1..];
                }

                Token::Bracket {
                    kind: BracketKind::Opening,
                    location,
                }
            }
            (b'[', location) => {
                depth += 1;
                input = &input[1..];

                Token::Bracket {
                    kind: BracketKind::Opening,
                    location,
                }
            }
            (b']', location) if depth > 0 => {
                depth -= 1;
                input = &input[1..];

                Token::Bracket {
                    kind: BracketKind::Closing,
                    location,
                }
            }
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
