#[allow(unused, deprecated)]
use std::ascii::AsciiExt;
use std::error::Error;
use std::fmt;
use std::iter::Enumerate;
use std::str::Bytes;

use super::{Mime, MimeIter, Source, ParamSource, Indexed, CHARSET, UTF_8};

#[derive(Debug)]
pub enum ParseError {
    MissingSlash,
    MissingEqual,
    MissingQuote,
    InvalidToken {
        pos: usize,
        byte: u8,
    },
}

impl ParseError {
    fn s(&self) -> &str {
        use self::ParseError::*;

        match *self {
            MissingSlash => "a slash (/) was missing between the type and subtype",
            MissingEqual => "an equals sign (=) was missing between a parameter and its value",
            MissingQuote => "a quote (\") was missing from a parameter value",
            InvalidToken { .. } => "an invalid token was encountered",
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let ParseError::InvalidToken { pos, byte } = *self {
            write!(f, "{}, {:X} at position {}", self.s(), byte, pos)
        } else {
            f.write_str(self.s())
        }
    }
}

impl Error for ParseError {
    // Minimum Rust is 1.15, Error::description was still required then
    #[allow(deprecated)]
    fn description(&self) -> &str {
        self.s()
    }
}

impl<'a> MimeIter<'a> {
    /// A new iterator over mimes or media types
    pub fn new(s: &'a str) -> Self {
        Self {
            pos: 0,
            source: s,
        }
    }
}

impl<'a> Iterator for MimeIter<'a> {
    type Item = Result<Mime, &'a str>;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.pos;
        let len = self.source.bytes().len();

        if start >= len {
            return None
        }

        // Try parsing the whole remaining slice, until the end
        match parse(&self.source[start ..len]) {
            Ok(value) => {
                self.pos = len;
                Some(Ok(value))
            }
            Err(ParseError::InvalidToken { pos, .. }) => {
                // The first token is immediately found to be wrong by `parse`. Skip it
                if pos == 0 {
                    self.pos += 1;
                    return self.next()
                }
                let slice = &self.source[start .. start + pos];
                // Try parsing the longest slice (until the first invalid token)
                return match parse(slice) {
                    Ok(mime) => {
                        self.pos = start + pos + 1;
                        Some(Ok(mime))
                    }
                    Err(_) => {
                        if start + pos < len {
                            // Skip this invalid slice,
                            // try parsing the remaining slice in the next iteration
                            self.pos = start + pos;
                            Some(Err(slice))
                        } else {
                            None
                        }
                    }
                }
            }
            // Do not process any other error condition: the slice is malformed and
            // no character is found to be invalid: a character is missing
            Err(_) => None,
        }
    }
}

pub fn parse(s: &str) -> Result<Mime, ParseError> {
    if s == "*/*" {
        return Ok(::STAR_STAR);
    }

    let mut iter = s.bytes().enumerate();
    // toplevel
    let mut start;
    let slash;
    loop {
        match iter.next() {
            Some((_, c)) if is_token(c) => (),
            Some((i, b'/')) if i > 0 => {
                slash = i;
                start = i + 1;
                break;
            },
            None => return Err(ParseError::MissingSlash), // EOF and no toplevel is no Mime
            Some((pos, byte)) => return Err(ParseError::InvalidToken {
                pos: pos,
                byte: byte,
            })
        };

    }

    // sublevel
    let mut plus = None;
    loop {
        match iter.next() {
            Some((i, b'+')) if i > start => {
                plus = Some(i);
            },
            Some((i, b';')) if i > start => {
                start = i;
                break;
            },
            Some((_, c)) if is_token(c) => (),
            None => {
                return Ok(Mime {
                    source: Source::Dynamic(s.to_ascii_lowercase()),
                    slash: slash,
                    plus: plus,
                    params: ParamSource::None,
                });
            },
            Some((pos, byte)) => return Err(ParseError::InvalidToken {
                pos: pos,
                byte: byte,
            })
        };
    }

    // params
    let params = params_from_str(s, &mut iter, start)?;

    let src = match params {
        ParamSource::Utf8(_)  => s.to_ascii_lowercase(),
        ParamSource::Custom(semicolon, ref indices) => lower_ascii_with_params(s, semicolon, indices),
        ParamSource::None => {
            // Chop off the empty list
            s[..start].to_ascii_lowercase()
        }
    };

    Ok(Mime {
        source: Source::Dynamic(src),
        slash: slash,
        plus: plus,
        params: params,
    })
}


fn params_from_str(s: &str, iter: &mut Enumerate<Bytes>, mut start: usize) -> Result<ParamSource, ParseError> {
    let semicolon = start;
    start += 1;
    let mut params = ParamSource::None;
    'params: while start < s.len() {
        let name;
        // name
        'name: loop {
            match iter.next() {
                Some((i, b' ')) if i == start => {
                    start = i + 1;
                    continue 'params;
                },
                Some((_, c)) if is_token(c) => (),
                Some((i, b'=')) if i > start => {
                    name = Indexed(start, i);
                    start = i + 1;
                    break 'name;
                },
                None => return Err(ParseError::MissingEqual),
                Some((pos, byte)) => return Err(ParseError::InvalidToken {
                    pos: pos,
                    byte: byte,
                }),
            }
        }

        let value;
        // values must be restrict-name-char or "anything goes"
        let mut is_quoted = false;

        'value: loop {
            if is_quoted {
                match iter.next() {
                    Some((i, b'"')) if i > start => {
                        value = Indexed(start, i);
                        break 'value;
                    },
                    Some((_, c)) if is_restricted_quoted_char(c) => (),
                    None => return Err(ParseError::MissingQuote),
                    Some((pos, byte)) => return Err(ParseError::InvalidToken {
                        pos: pos,
                        byte: byte,
                    }),
                }
            } else {
                match iter.next() {
                    Some((i, b'"')) if i == start => {
                        is_quoted = true;
                        start = i + 1;
                    },
                    Some((_, c)) if is_token(c) => (),
                    Some((i, b';')) if i > start => {
                        value = Indexed(start, i);
                        start = i + 1;
                        break 'value;
                    }
                    None => {
                        value = Indexed(start, s.len());
                        start = s.len();
                        break 'value;
                    },

                    Some((pos, byte)) => return Err(ParseError::InvalidToken {
                        pos: pos,
                        byte: byte,
                    }),
                }
            }
        }

        if is_quoted {
            'ws: loop {
                match iter.next() {
                    Some((i, b';')) => {
                        // next param
                        start = i + 1;
                        break 'ws;
                    },
                    Some((_, b' ')) => {
                        // skip whitespace
                    },
                    None => {
                        // eof
                        start = s.len();
                        break 'ws;
                    },
                    Some((pos, byte)) => return Err(ParseError::InvalidToken {
                        pos: pos,
                        byte: byte,
                    }),
                }
            }
        }

        match params {
            ParamSource::Utf8(i) => {
                let i = i + 2;
                let charset = Indexed(i, "charset".len() + i);
                let utf8 = Indexed(charset.1 + 1, charset.1 + "utf-8".len() + 1);
                params = ParamSource::Custom(semicolon, vec![
                    (charset, utf8),
                    (name, value),
                ]);
            },
            ParamSource::Custom(_, ref mut vec) => {
                vec.push((name, value));
            },
            ParamSource::None => {
                if semicolon + 2 == name.0 && CHARSET == &s[name.0..name.1] {
                    if UTF_8 == &s[value.0..value.1] {
                        params = ParamSource::Utf8(semicolon);
                        continue 'params;
                    }
                }
                params = ParamSource::Custom(semicolon, vec![(name, value)]);
            },
        }
    }
    Ok(params)
}

fn lower_ascii_with_params(s: &str, semi: usize, params: &[(Indexed, Indexed)]) -> String {
    let mut owned = s.to_owned();
    owned[..semi].make_ascii_lowercase();

    for &(ref name, ref value) in params {
        owned[name.0..name.1].make_ascii_lowercase();
        // Since we just converted this part of the string to lowercase,
        // we can skip the `Name == &str` unicase check and do a faster
        // memcmp instead.
        if &owned[name.0..name.1] == CHARSET.source {
            owned[value.0..value.1].make_ascii_lowercase();
        }
    }

    owned
}

// From [RFC6838](http://tools.ietf.org/html/rfc6838#section-4.2):
//
// > All registered media types MUST be assigned top-level type and
// > subtype names.  The combination of these names serves to uniquely
// > identify the media type, and the subtype name facet (or the absence
// > of one) identifies the registration tree.  Both top-level type and
// > subtype names are case-insensitive.
// >
// > Type and subtype names MUST conform to the following ABNF:
// >
// >     type-name = restricted-name
// >     subtype-name = restricted-name
// >
// >     restricted-name = restricted-name-first *126restricted-name-chars
// >     restricted-name-first  = ALPHA / DIGIT
// >     restricted-name-chars  = ALPHA / DIGIT / "!" / "#" /
// >                              "$" / "&" / "-" / "^" / "_"
// >     restricted-name-chars =/ "." ; Characters before first dot always
// >                                  ; specify a facet name
// >     restricted-name-chars =/ "+" ; Characters after last plus always
// >                                  ; specify a structured syntax suffix

// However, [HTTP](https://tools.ietf.org/html/rfc7231#section-3.1.1.1):
//
// >     media-type = type "/" subtype *( OWS ";" OWS parameter )
// >     type       = token
// >     subtype    = token
// >     parameter  = token "=" ( token / quoted-string )
//
// Where token is defined as:
//
// >     token = 1*tchar
// >     tchar = "!" / "#" / "$" / "%" / "&" / "'" / "*" / "+" / "-" / "." /
// >        "^" / "_" / "`" / "|" / "~" / DIGIT / ALPHA
//
// So, clearly, ¯\_(Ä_/¯

macro_rules! byte_map {
    ($($flag:expr,)*) => ([
        $($flag != 0,)*
    ])
}

static TOKEN_MAP: [bool; 256] = byte_map![
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 1, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 1, 1, 0,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0,
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

fn is_token(c: u8) -> bool {
    TOKEN_MAP[c as usize]
}

fn is_restricted_quoted_char(c: u8) -> bool {
    c > 31 && c != 127
}

#[test]
#[allow(warnings)] // ... ranges deprecated
fn test_lookup_tables() {
    for (i, &valid) in TOKEN_MAP.iter().enumerate() {
        let i = i as u8;
        let should = match i {
            b'a'...b'z' |
            b'A'...b'Z' |
            b'0'...b'9' |
            b'!' |
            b'#' |
            b'$' |
            b'%' |
            b'&' |
            b'\'' |
            b'*' |
            b'+' |
            b'-' |
            b'.' |
            b'^' |
            b'_' |
            b'`' |
            b'|' |
            b'~' => true,
            _ => false
        };
        assert_eq!(valid, should, "{:?} ({}) should be {}", i as char, i, should);
    }
}

#[test]
fn test_parse_iterator() {
    let mut iter = MimeIter::new("application/json, application/json");
    assert_eq!(iter.next().unwrap().unwrap(), parse("application/json").unwrap());
    assert_eq!(iter.next().unwrap().unwrap(), parse("application/json").unwrap());
    assert_eq!(iter.next(), None);

    let mut iter = MimeIter::new("application/json");
    assert_eq!(iter.next().unwrap().unwrap(), parse("application/json").unwrap());
    assert_eq!(iter.next(), None);

    let mut iter = MimeIter::new("application/json;  ");
    assert_eq!(iter.next().unwrap().unwrap(), parse("application/json").unwrap());
    assert_eq!(iter.next(), None);
}

#[test]
fn test_parse_iterator_invalid() {
    let mut iter = MimeIter::new("application/json, invalid, application/json");
    assert_eq!(iter.next().unwrap().unwrap(), parse("application/json").unwrap());
    assert_eq!(iter.next().unwrap().unwrap_err(), "invalid");
    assert_eq!(iter.next().unwrap().unwrap(), parse("application/json").unwrap());
    assert_eq!(iter.next(), None);
}

#[test]
fn test_parse_iterator_all_invalid() {
    let mut iter = MimeIter::new("application/json, text/html");
    assert_eq!(iter.next().unwrap().unwrap_err(), "application/json");
    assert_eq!(iter.next(), None);
}
