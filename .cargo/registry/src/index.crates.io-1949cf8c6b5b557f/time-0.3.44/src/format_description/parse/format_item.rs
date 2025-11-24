//! Typed, validated representation of a parsed format description.

use alloc::boxed::Box;
use alloc::string::String;
use core::num::NonZero;
use core::str::{self, FromStr};

use super::{ast, unused, Error, Span, Spanned};
use crate::internal_macros::bug;

/// Parse an AST iterator into a sequence of format items.
#[inline]
pub(super) fn parse<'a>(
    ast_items: impl Iterator<Item = Result<ast::Item<'a>, Error>>,
) -> impl Iterator<Item = Result<Item<'a>, Error>> {
    ast_items.map(|ast_item| ast_item.and_then(Item::from_ast))
}

/// A description of how to format and parse one part of a type.
pub(super) enum Item<'a> {
    /// A literal string.
    Literal(&'a [u8]),
    /// Part of a type, along with its modifiers.
    Component(Component),
    /// A sequence of optional items.
    Optional {
        /// The items themselves.
        value: Box<[Self]>,
        /// The span of the full sequence.
        span: Span,
    },
    /// The first matching parse of a sequence of format descriptions.
    First {
        /// The sequence of format descriptions.
        value: Box<[Box<[Self]>]>,
        /// The span of the full sequence.
        span: Span,
    },
}

impl Item<'_> {
    /// Parse an AST item into a format item.
    pub(super) fn from_ast(ast_item: ast::Item<'_>) -> Result<Item<'_>, Error> {
        Ok(match ast_item {
            ast::Item::Component {
                _opening_bracket: _,
                _leading_whitespace: _,
                name,
                modifiers,
                _trailing_whitespace: _,
                _closing_bracket: _,
            } => Item::Component(component_from_ast(&name, &modifiers)?),
            ast::Item::Literal(Spanned { value, span: _ }) => Item::Literal(value),
            ast::Item::EscapedBracket {
                _first: _,
                _second: _,
            } => Item::Literal(b"["),
            ast::Item::Optional {
                opening_bracket,
                _leading_whitespace: _,
                _optional_kw: _,
                _whitespace: _,
                nested_format_description,
                closing_bracket,
            } => {
                let items = nested_format_description
                    .items
                    .into_vec()
                    .into_iter()
                    .map(Item::from_ast)
                    .collect::<Result<_, _>>()?;
                Item::Optional {
                    value: items,
                    span: opening_bracket.to(closing_bracket),
                }
            }
            ast::Item::First {
                opening_bracket,
                _leading_whitespace: _,
                _first_kw: _,
                _whitespace: _,
                nested_format_descriptions,
                closing_bracket,
            } => {
                let items = nested_format_descriptions
                    .into_vec()
                    .into_iter()
                    .map(|nested_format_description| {
                        nested_format_description
                            .items
                            .into_vec()
                            .into_iter()
                            .map(Item::from_ast)
                            .collect()
                    })
                    .collect::<Result<_, _>>()?;
                Item::First {
                    value: items,
                    span: opening_bracket.to(closing_bracket),
                }
            }
        })
    }
}

impl<'a> TryFrom<Item<'a>> for crate::format_description::BorrowedFormatItem<'a> {
    type Error = Error;

    #[inline]
    fn try_from(item: Item<'a>) -> Result<Self, Self::Error> {
        match item {
            Item::Literal(literal) => Ok(Self::Literal(literal)),
            Item::Component(component) => Ok(Self::Component(component.into())),
            Item::Optional { value: _, span } => Err(Error {
                _inner: unused(span.error(
                    "optional items are not supported in runtime-parsed format descriptions",
                )),
                public: crate::error::InvalidFormatDescription::NotSupported {
                    what: "optional item",
                    context: "runtime-parsed format descriptions",
                    index: span.start.byte as usize,
                },
            }),
            Item::First { value: _, span } => Err(Error {
                _inner: unused(span.error(
                    "'first' items are not supported in runtime-parsed format descriptions",
                )),
                public: crate::error::InvalidFormatDescription::NotSupported {
                    what: "'first' item",
                    context: "runtime-parsed format descriptions",
                    index: span.start.byte as usize,
                },
            }),
        }
    }
}

impl From<Item<'_>> for crate::format_description::OwnedFormatItem {
    #[inline]
    fn from(item: Item<'_>) -> Self {
        match item {
            Item::Literal(literal) => Self::Literal(literal.to_vec().into_boxed_slice()),
            Item::Component(component) => Self::Component(component.into()),
            Item::Optional { value, span: _ } => Self::Optional(Box::new(value.into())),
            Item::First { value, span: _ } => {
                Self::First(value.into_vec().into_iter().map(Into::into).collect())
            }
        }
    }
}

impl<'a> From<Box<[Item<'a>]>> for crate::format_description::OwnedFormatItem {
    #[inline]
    fn from(items: Box<[Item<'a>]>) -> Self {
        let items = items.into_vec();
        match <[_; 1]>::try_from(items) {
            Ok([item]) => item.into(),
            Err(vec) => Self::Compound(vec.into_iter().map(Into::into).collect()),
        }
    }
}

/// Declare the `Component` struct.
macro_rules! component_definition {
    (@if_required required then { $($then:tt)* } $(else { $($else:tt)* })?) => { $($then)* };
    (@if_required then { $($then:tt)* } $(else { $($else:tt)* })?) => { $($($else)*)? };
    (@if_from_str from_str then { $($then:tt)* } $(else { $($else:tt)* })?) => { $($then)* };
    (@if_from_str then { $($then:tt)* } $(else { $($else:tt)* })?) => { $($($else)*)? };

    ($vis:vis enum $name:ident {
        $($variant:ident = $parse_variant:literal {$(
            $(#[$required:tt])?
            $field:ident = $parse_field:literal:
            Option<$(#[$from_str:tt])? $field_type:ty>
            => $target_field:ident
        ),* $(,)?}),* $(,)?
    }) => {
        $vis enum $name {
            $($variant($variant),)*
        }

        $($vis struct $variant {
            $($field: Option<$field_type>),*
        })*

        $(impl $variant {
            /// Parse the component from the AST, given its modifiers.
            #[inline]
            fn with_modifiers(
                modifiers: &[ast::Modifier<'_>],
                _component_span: Span,
            ) -> Result<Self, Error>
            {
                // rustc will complain if the modifier is empty.
                #[allow(unused_mut)]
                let mut this = Self {
                    $($field: None),*
                };

                for modifier in modifiers {
                    $(#[expect(clippy::string_lit_as_bytes)]
                    if modifier.key.eq_ignore_ascii_case($parse_field.as_bytes()) {
                        this.$field = component_definition!(@if_from_str $($from_str)?
                            then {
                                parse_from_modifier_value::<$field_type>(&modifier.value)?
                            } else {
                                <$field_type>::from_modifier_value(&modifier.value)?
                            });
                        continue;
                    })*
                    return Err(Error {
                        _inner: unused(modifier.key.span.error("invalid modifier key")),
                        public: crate::error::InvalidFormatDescription::InvalidModifier {
                            value: String::from_utf8_lossy(*modifier.key).into_owned(),
                            index: modifier.key.span.start.byte as usize,
                        }
                    });
                }

                $(component_definition! { @if_required $($required)? then {
                    if this.$field.is_none() {
                        return Err(Error {
                            _inner: unused(_component_span.error("missing required modifier")),
                            public:
                                crate::error::InvalidFormatDescription::MissingRequiredModifier {
                                    name: $parse_field,
                                    index: _component_span.start.byte as usize,
                                }
                        });
                    }
                }})*

                Ok(this)
            }
        })*

        impl From<$name> for crate::format_description::Component {
            #[inline]
            fn from(component: $name) -> Self {
                match component {$(
                    $name::$variant($variant { $($field),* }) => {
                        $crate::format_description::component::Component::$variant(
                            $crate::format_description::modifier::$variant {$(
                                $target_field: component_definition! { @if_required $($required)?
                                    then {
                                        match $field {
                                            Some(value) => value.into(),
                                            None => bug!("required modifier was not set"),
                                        }
                                    } else {
                                        $field.unwrap_or_default().into()
                                    }
                                }
                            ),*}
                        )
                    }
                )*}
            }
        }

        /// Parse a component from the AST, given its name and modifiers.
        #[inline]
        fn component_from_ast(
            name: &Spanned<&[u8]>,
            modifiers: &[ast::Modifier<'_>],
        ) -> Result<Component, Error> {
            $(#[expect(clippy::string_lit_as_bytes)]
            if name.eq_ignore_ascii_case($parse_variant.as_bytes()) {
                return Ok(Component::$variant($variant::with_modifiers(&modifiers, name.span)?));
            })*
            Err(Error {
                _inner: unused(name.span.error("invalid component")),
                public: crate::error::InvalidFormatDescription::InvalidComponentName {
                    name: String::from_utf8_lossy(name).into_owned(),
                    index: name.span.start.byte as usize,
                },
            })
        }
    }
}

// Keep in alphabetical order.
component_definition! {
    pub(super) enum Component {
        Day = "day" {
            padding = "padding": Option<Padding> => padding,
        },
        End = "end" {},
        Hour = "hour" {
            padding = "padding": Option<Padding> => padding,
            base = "repr": Option<HourBase> => is_12_hour_clock,
        },
        Ignore = "ignore" {
            #[required]
            count = "count": Option<#[from_str] NonZero<u16>> => count,
        },
        Minute = "minute" {
            padding = "padding": Option<Padding> => padding,
        },
        Month = "month" {
            padding = "padding": Option<Padding> => padding,
            repr = "repr": Option<MonthRepr> => repr,
            case_sensitive = "case_sensitive": Option<MonthCaseSensitive> => case_sensitive,
        },
        OffsetHour = "offset_hour" {
            sign_behavior = "sign": Option<SignBehavior> => sign_is_mandatory,
            padding = "padding": Option<Padding> => padding,
        },
        OffsetMinute = "offset_minute" {
            padding = "padding": Option<Padding> => padding,
        },
        OffsetSecond = "offset_second" {
            padding = "padding": Option<Padding> => padding,
        },
        Ordinal = "ordinal" {
            padding = "padding": Option<Padding> => padding,
        },
        Period = "period" {
            case = "case": Option<PeriodCase> => is_uppercase,
            case_sensitive = "case_sensitive": Option<PeriodCaseSensitive> => case_sensitive,
        },
        Second = "second" {
            padding = "padding": Option<Padding> => padding,
        },
        Subsecond = "subsecond" {
            digits = "digits": Option<SubsecondDigits> => digits,
        },
        UnixTimestamp = "unix_timestamp" {
            precision = "precision": Option<UnixTimestampPrecision> => precision,
            sign_behavior = "sign": Option<SignBehavior> => sign_is_mandatory,
        },
        Weekday = "weekday" {
            repr = "repr": Option<WeekdayRepr> => repr,
            one_indexed = "one_indexed": Option<WeekdayOneIndexed> => one_indexed,
            case_sensitive = "case_sensitive": Option<WeekdayCaseSensitive> => case_sensitive,
        },
        WeekNumber = "week_number" {
            padding = "padding": Option<Padding> => padding,
            repr = "repr": Option<WeekNumberRepr> => repr,
        },
        Year = "year" {
            padding = "padding": Option<Padding> => padding,
            repr = "repr": Option<YearRepr> => repr,
            range = "range": Option<YearRange> => range,
            base = "base": Option<YearBase> => iso_week_based,
            sign_behavior = "sign": Option<SignBehavior> => sign_is_mandatory,
        },
    }
}

/// Get the target type for a given enum.
macro_rules! target_ty {
    ($name:ident $type:ty) => {
        $type
    };
    ($name:ident) => {
        $crate::format_description::modifier::$name
    };
}

/// Get the target value for a given enum.
macro_rules! target_value {
    ($name:ident $variant:ident $value:expr) => {
        $value
    };
    ($name:ident $variant:ident) => {
        $crate::format_description::modifier::$name::$variant
    };
}

/// Declare the various modifiers.
///
/// For the general case, ordinary syntax can be used. Note that you _must_ declare a default
/// variant. The only significant change is that the string representation of the variant must be
/// provided after the variant name. For example, `Numerical = b"numerical"` declares a variant
/// named `Numerical` with the string representation `b"numerical"`. This is the value that will be
/// used when parsing the modifier. The value is not case sensitive.
///
/// If the type in the public API does not have the same name as the type in the internal
/// representation, then the former must be specified in parenthesis after the internal name. For
/// example, `HourBase(bool)` has an internal name "HourBase", but is represented as a boolean in
/// the public API.
///
/// By default, the internal variant name is assumed to be the same as the public variant name. If
/// this is not the case, the qualified path to the variant must be specified in parenthesis after
/// the internal variant name. For example, `Twelve(true)` has an internal variant name "Twelve",
/// but is represented as `true` in the public API.
macro_rules! modifier {
    ($(
        enum $name:ident $(($target_ty:ty))? {
            $(
                $(#[$attr:meta])?
                $variant:ident $(($target_value:expr))? = $parse_variant:literal
            ),* $(,)?
        }
    )+) => {$(
        #[derive(Default)]
        enum $name {
            $($(#[$attr])? $variant),*
        }

        impl $name {
            /// Parse the modifier from its string representation.
            #[inline]
            fn from_modifier_value(value: &Spanned<&[u8]>) -> Result<Option<Self>, Error> {
                $(if value.eq_ignore_ascii_case($parse_variant) {
                    return Ok(Some(Self::$variant));
                })*
                Err(Error {
                    _inner: unused(value.span.error("invalid modifier value")),
                    public: crate::error::InvalidFormatDescription::InvalidModifier {
                        value: String::from_utf8_lossy(value).into_owned(),
                        index: value.span.start.byte as usize,
                    },
                })
            }
        }

        impl From<$name> for target_ty!($name $($target_ty)?) {
            #[inline]
            fn from(modifier: $name) -> Self {
                match modifier {
                    $($name::$variant => target_value!($name $variant $($target_value)?)),*
                }
            }
        }
    )+};
}

// Keep in alphabetical order.
modifier! {
    enum HourBase(bool) {
        Twelve(true) = b"12",
        #[default]
        TwentyFour(false) = b"24",
    }

    enum MonthCaseSensitive(bool) {
        False(false) = b"false",
        #[default]
        True(true) = b"true",
    }

    enum MonthRepr {
        #[default]
        Numerical = b"numerical",
        Long = b"long",
        Short = b"short",
    }

    enum Padding {
        Space = b"space",
        #[default]
        Zero = b"zero",
        None = b"none",
    }

    enum PeriodCase(bool) {
        Lower(false) = b"lower",
        #[default]
        Upper(true) = b"upper",
    }

    enum PeriodCaseSensitive(bool) {
        False(false) = b"false",
        #[default]
        True(true) = b"true",
    }

    enum SignBehavior(bool) {
        #[default]
        Automatic(false) = b"automatic",
        Mandatory(true) = b"mandatory",
    }

    enum SubsecondDigits {
        One = b"1",
        Two = b"2",
        Three = b"3",
        Four = b"4",
        Five = b"5",
        Six = b"6",
        Seven = b"7",
        Eight = b"8",
        Nine = b"9",
        #[default]
        OneOrMore = b"1+",
    }

    enum UnixTimestampPrecision {
        #[default]
        Second = b"second",
        Millisecond = b"millisecond",
        Microsecond = b"microsecond",
        Nanosecond = b"nanosecond",
    }

    enum WeekNumberRepr {
        #[default]
        Iso = b"iso",
        Sunday = b"sunday",
        Monday = b"monday",
    }

    enum WeekdayCaseSensitive(bool) {
        False(false) = b"false",
        #[default]
        True(true) = b"true",
    }

    enum WeekdayOneIndexed(bool) {
        False(false) = b"false",
        #[default]
        True(true) = b"true",
    }

    enum WeekdayRepr {
        Short = b"short",
        #[default]
        Long = b"long",
        Sunday = b"sunday",
        Monday = b"monday",
    }

    enum YearBase(bool) {
        #[default]
        Calendar(false) = b"calendar",
        IsoWeek(true) = b"iso_week",
    }

    enum YearRepr {
        #[default]
        Full = b"full",
        Century = b"century",
        LastTwo = b"last_two",
    }

    enum YearRange {
        Standard = b"standard",
        #[default]
        Extended = b"extended",
    }
}

/// Parse a modifier value using `FromStr`. Requires the modifier value to be valid UTF-8.
#[inline]
fn parse_from_modifier_value<T: FromStr>(value: &Spanned<&[u8]>) -> Result<Option<T>, Error> {
    str::from_utf8(value)
        .ok()
        .and_then(|val| val.parse::<T>().ok())
        .map(|val| Some(val))
        .ok_or_else(|| Error {
            _inner: unused(value.span.error("invalid modifier value")),
            public: crate::error::InvalidFormatDescription::InvalidModifier {
                value: String::from_utf8_lossy(value).into_owned(),
                index: value.span.start.byte as usize,
            },
        })
}
