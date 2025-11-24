use alloc::string::String;
use alloc::vec::Vec;
use core::iter;

use crate::error::InvalidFormatDescription;
use crate::format_description::parse::{
    attach_location, unused, Error, ErrorInner, Location, Spanned, SpannedValue, Unused,
};
use crate::format_description::{self, modifier, BorrowedFormatItem, Component};

/// Parse a sequence of items from the [`strftime` format description][strftime docs].
///
/// The only heap allocation required is for the `Vec` itself. All components are bound to the
/// lifetime of the input.
///
/// [strftime docs]: https://man7.org/linux/man-pages/man3/strftime.3.html
#[doc(alias = "parse_strptime_borrowed")]
#[inline]
pub fn parse_strftime_borrowed(
    s: &str,
) -> Result<Vec<BorrowedFormatItem<'_>>, InvalidFormatDescription> {
    let tokens = lex(s.as_bytes());
    let items = into_items(tokens).collect::<Result<_, _>>()?;
    Ok(items)
}

/// Parse a sequence of items from the [`strftime` format description][strftime docs].
///
/// This requires heap allocation for some owned items.
///
/// [strftime docs]: https://man7.org/linux/man-pages/man3/strftime.3.html
#[doc(alias = "parse_strptime_owned")]
#[inline]
pub fn parse_strftime_owned(
    s: &str,
) -> Result<format_description::OwnedFormatItem, InvalidFormatDescription> {
    parse_strftime_borrowed(s).map(Into::into)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Padding {
    /// The default padding for a numeric component. Indicated by no character.
    Default,
    /// Pad a numeric component with spaces. Indicated by an underscore.
    Spaces,
    /// Do not pad a numeric component. Indicated by a hyphen.
    None,
    /// Pad a numeric component with zeroes. Indicated by a zero.
    Zeroes,
}

enum Token<'a> {
    Literal(Spanned<&'a [u8]>),
    Component {
        _percent: Unused<Location>,
        padding: Spanned<Padding>,
        component: Spanned<u8>,
    },
}

#[inline]
fn lex(mut input: &[u8]) -> iter::Peekable<impl Iterator<Item = Result<Token<'_>, Error>>> {
    let mut iter = attach_location(input.iter()).peekable();

    iter::from_fn(move || {
        Some(Ok(match iter.next()? {
            (b'%', percent_loc) => match iter.next() {
                Some((padding @ (b'_' | b'-' | b'0'), padding_loc)) => {
                    let padding = match padding {
                        b'_' => Padding::Spaces,
                        b'-' => Padding::None,
                        b'0' => Padding::Zeroes,
                        _ => unreachable!(),
                    };
                    let (&component, component_loc) = iter.next()?;
                    input = &input[3..];
                    Token::Component {
                        _percent: unused(percent_loc),
                        padding: padding.spanned(padding_loc.to_self()),
                        component: component.spanned(component_loc.to_self()),
                    }
                }
                Some((&component, component_loc)) => {
                    input = &input[2..];
                    let span = component_loc.to_self();
                    Token::Component {
                        _percent: unused(percent_loc),
                        padding: Padding::Default.spanned(span),
                        component: component.spanned(span),
                    }
                }
                None => {
                    return Some(Err(Error {
                        _inner: unused(percent_loc.error("unexpected end of input")),
                        public: InvalidFormatDescription::Expected {
                            what: "valid escape sequence",
                            index: percent_loc.byte as usize,
                        },
                    }));
                }
            },
            (_, start_location) => {
                let mut bytes = 1;
                let mut end_location = start_location;

                while let Some((_, location)) = iter.next_if(|&(&byte, _)| byte != b'%') {
                    end_location = location;
                    bytes += 1;
                }

                let value = &input[..bytes];
                input = &input[bytes..];

                Token::Literal(value.spanned(start_location.to(end_location)))
            }
        }))
    })
    .peekable()
}

#[inline]
fn into_items<'iter, 'token: 'iter>(
    mut tokens: iter::Peekable<impl Iterator<Item = Result<Token<'token>, Error>> + 'iter>,
) -> impl Iterator<Item = Result<BorrowedFormatItem<'token>, Error>> + 'iter {
    iter::from_fn(move || {
        let next = match tokens.next()? {
            Ok(token) => token,
            Err(err) => return Some(Err(err)),
        };

        Some(match next {
            Token::Literal(spanned) => Ok(BorrowedFormatItem::Literal(*spanned)),
            Token::Component {
                _percent,
                padding,
                component,
            } => parse_component(padding, component),
        })
    })
}

fn parse_component(
    padding: Spanned<Padding>,
    component: Spanned<u8>,
) -> Result<BorrowedFormatItem<'static>, Error> {
    let padding_or_default = |padding: Padding, default| match padding {
        Padding::Default => default,
        Padding::Spaces => modifier::Padding::Space,
        Padding::None => modifier::Padding::None,
        Padding::Zeroes => modifier::Padding::Zero,
    };

    /// Helper macro to create a component.
    macro_rules! component {
        ($name:ident { $($inner:tt)* }) => {
            BorrowedFormatItem::Component(Component::$name(modifier::$name {
                $($inner)*
            }))
        }
    }

    Ok(match *component {
        b'%' => BorrowedFormatItem::Literal(b"%"),
        b'a' => component!(Weekday {
            repr: modifier::WeekdayRepr::Short,
            one_indexed: true,
            case_sensitive: true,
        }),
        b'A' => component!(Weekday {
            repr: modifier::WeekdayRepr::Long,
            one_indexed: true,
            case_sensitive: true,
        }),
        b'b' | b'h' => component!(Month {
            repr: modifier::MonthRepr::Short,
            padding: modifier::Padding::Zero,
            case_sensitive: true,
        }),
        b'B' => component!(Month {
            repr: modifier::MonthRepr::Long,
            padding: modifier::Padding::Zero,
            case_sensitive: true,
        }),
        b'c' => BorrowedFormatItem::Compound(&[
            component!(Weekday {
                repr: modifier::WeekdayRepr::Short,
                one_indexed: true,
                case_sensitive: true,
            }),
            BorrowedFormatItem::Literal(b" "),
            component!(Month {
                repr: modifier::MonthRepr::Short,
                padding: modifier::Padding::Zero,
                case_sensitive: true,
            }),
            BorrowedFormatItem::Literal(b" "),
            component!(Day {
                padding: modifier::Padding::Space
            }),
            BorrowedFormatItem::Literal(b" "),
            component!(Hour {
                padding: modifier::Padding::Zero,
                is_12_hour_clock: false,
            }),
            BorrowedFormatItem::Literal(b":"),
            component!(Minute {
                padding: modifier::Padding::Zero,
            }),
            BorrowedFormatItem::Literal(b":"),
            component!(Second {
                padding: modifier::Padding::Zero,
            }),
            BorrowedFormatItem::Literal(b" "),
            component!(Year {
                padding: modifier::Padding::Zero,
                repr: modifier::YearRepr::Full,
                range: modifier::YearRange::Extended,
                iso_week_based: false,
                sign_is_mandatory: false,
            }),
        ]),
        b'C' => component!(Year {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
            repr: modifier::YearRepr::Century,
            range: modifier::YearRange::Extended,
            iso_week_based: false,
            sign_is_mandatory: false,
        }),
        b'd' => component!(Day {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
        }),
        b'D' => BorrowedFormatItem::Compound(&[
            component!(Month {
                repr: modifier::MonthRepr::Numerical,
                padding: modifier::Padding::Zero,
                case_sensitive: true,
            }),
            BorrowedFormatItem::Literal(b"/"),
            component!(Day {
                padding: modifier::Padding::Zero,
            }),
            BorrowedFormatItem::Literal(b"/"),
            component!(Year {
                padding: modifier::Padding::Zero,
                repr: modifier::YearRepr::LastTwo,
                range: modifier::YearRange::Extended,
                iso_week_based: false,
                sign_is_mandatory: false,
            }),
        ]),
        b'e' => component!(Day {
            padding: padding_or_default(*padding, modifier::Padding::Space),
        }),
        b'F' => BorrowedFormatItem::Compound(&[
            component!(Year {
                padding: modifier::Padding::Zero,
                repr: modifier::YearRepr::Full,
                range: modifier::YearRange::Extended,
                iso_week_based: false,
                sign_is_mandatory: false,
            }),
            BorrowedFormatItem::Literal(b"-"),
            component!(Month {
                padding: modifier::Padding::Zero,
                repr: modifier::MonthRepr::Numerical,
                case_sensitive: true,
            }),
            BorrowedFormatItem::Literal(b"-"),
            component!(Day {
                padding: modifier::Padding::Zero,
            }),
        ]),
        b'g' => component!(Year {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
            repr: modifier::YearRepr::LastTwo,
            range: modifier::YearRange::Extended,
            iso_week_based: true,
            sign_is_mandatory: false,
        }),
        b'G' => component!(Year {
            padding: modifier::Padding::Zero,
            repr: modifier::YearRepr::Full,
            range: modifier::YearRange::Extended,
            iso_week_based: true,
            sign_is_mandatory: false,
        }),
        b'H' => component!(Hour {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
            is_12_hour_clock: false,
        }),
        b'I' => component!(Hour {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
            is_12_hour_clock: true,
        }),
        b'j' => component!(Ordinal {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
        }),
        b'k' => component!(Hour {
            padding: padding_or_default(*padding, modifier::Padding::Space),
            is_12_hour_clock: false,
        }),
        b'l' => component!(Hour {
            padding: padding_or_default(*padding, modifier::Padding::Space),
            is_12_hour_clock: true,
        }),
        b'm' => component!(Month {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
            repr: modifier::MonthRepr::Numerical,
            case_sensitive: true,
        }),
        b'M' => component!(Minute {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
        }),
        b'n' => BorrowedFormatItem::Literal(b"\n"),
        b'O' => {
            return Err(Error {
                _inner: unused(ErrorInner {
                    _message: "unsupported modifier",
                    _span: component.span,
                }),
                public: InvalidFormatDescription::NotSupported {
                    what: "modifier",
                    context: "",
                    index: component.span.start.byte as usize,
                },
            })
        }
        b'p' => component!(Period {
            is_uppercase: true,
            case_sensitive: true
        }),
        b'P' => component!(Period {
            is_uppercase: false,
            case_sensitive: true
        }),
        b'r' => BorrowedFormatItem::Compound(&[
            component!(Hour {
                padding: modifier::Padding::Zero,
                is_12_hour_clock: true,
            }),
            BorrowedFormatItem::Literal(b":"),
            component!(Minute {
                padding: modifier::Padding::Zero,
            }),
            BorrowedFormatItem::Literal(b":"),
            component!(Second {
                padding: modifier::Padding::Zero,
            }),
            BorrowedFormatItem::Literal(b" "),
            component!(Period {
                is_uppercase: true,
                case_sensitive: true,
            }),
        ]),
        b'R' => BorrowedFormatItem::Compound(&[
            component!(Hour {
                padding: modifier::Padding::Zero,
                is_12_hour_clock: false,
            }),
            BorrowedFormatItem::Literal(b":"),
            component!(Minute {
                padding: modifier::Padding::Zero,
            }),
        ]),
        b's' => component!(UnixTimestamp {
            precision: modifier::UnixTimestampPrecision::Second,
            sign_is_mandatory: false,
        }),
        b'S' => component!(Second {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
        }),
        b't' => BorrowedFormatItem::Literal(b"\t"),
        b'T' => BorrowedFormatItem::Compound(&[
            component!(Hour {
                padding: modifier::Padding::Zero,
                is_12_hour_clock: false,
            }),
            BorrowedFormatItem::Literal(b":"),
            component!(Minute {
                padding: modifier::Padding::Zero,
            }),
            BorrowedFormatItem::Literal(b":"),
            component!(Second {
                padding: modifier::Padding::Zero,
            }),
        ]),
        b'u' => component!(Weekday {
            repr: modifier::WeekdayRepr::Monday,
            one_indexed: true,
            case_sensitive: true,
        }),
        b'U' => component!(WeekNumber {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
            repr: modifier::WeekNumberRepr::Sunday,
        }),
        b'V' => component!(WeekNumber {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
            repr: modifier::WeekNumberRepr::Iso,
        }),
        b'w' => component!(Weekday {
            repr: modifier::WeekdayRepr::Sunday,
            one_indexed: true,
            case_sensitive: true,
        }),
        b'W' => component!(WeekNumber {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
            repr: modifier::WeekNumberRepr::Monday,
        }),
        b'x' => BorrowedFormatItem::Compound(&[
            component!(Month {
                repr: modifier::MonthRepr::Numerical,
                padding: modifier::Padding::Zero,
                case_sensitive: true,
            }),
            BorrowedFormatItem::Literal(b"/"),
            component!(Day {
                padding: modifier::Padding::Zero
            }),
            BorrowedFormatItem::Literal(b"/"),
            component!(Year {
                padding: modifier::Padding::Zero,
                repr: modifier::YearRepr::LastTwo,
                range: modifier::YearRange::Extended,
                iso_week_based: false,
                sign_is_mandatory: false,
            }),
        ]),
        b'X' => BorrowedFormatItem::Compound(&[
            component!(Hour {
                padding: modifier::Padding::Zero,
                is_12_hour_clock: false,
            }),
            BorrowedFormatItem::Literal(b":"),
            component!(Minute {
                padding: modifier::Padding::Zero,
            }),
            BorrowedFormatItem::Literal(b":"),
            component!(Second {
                padding: modifier::Padding::Zero,
            }),
        ]),
        b'y' => component!(Year {
            padding: padding_or_default(*padding, modifier::Padding::Zero),
            repr: modifier::YearRepr::LastTwo,
            range: modifier::YearRange::Extended,
            iso_week_based: false,
            sign_is_mandatory: false,
        }),
        b'Y' => component!(Year {
            padding: modifier::Padding::Zero,
            repr: modifier::YearRepr::Full,
            range: modifier::YearRange::Extended,
            iso_week_based: false,
            sign_is_mandatory: false,
        }),
        b'z' => BorrowedFormatItem::Compound(&[
            component!(OffsetHour {
                sign_is_mandatory: true,
                padding: modifier::Padding::Zero,
            }),
            component!(OffsetMinute {
                padding: modifier::Padding::Zero,
            }),
        ]),
        b'Z' => {
            return Err(Error {
                _inner: unused(ErrorInner {
                    _message: "unsupported component",
                    _span: component.span,
                }),
                public: InvalidFormatDescription::NotSupported {
                    what: "component",
                    context: "",
                    index: component.span.start.byte as usize,
                },
            })
        }
        _ => {
            return Err(Error {
                _inner: unused(ErrorInner {
                    _message: "invalid component",
                    _span: component.span,
                }),
                public: InvalidFormatDescription::InvalidComponentName {
                    name: String::from_utf8_lossy(&[*component]).into_owned(),
                    index: component.span.start.byte as usize,
                },
            })
        }
    })
}
