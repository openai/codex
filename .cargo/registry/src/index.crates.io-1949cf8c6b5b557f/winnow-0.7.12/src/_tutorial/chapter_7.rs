//! # Chapter 7: Error Reporting
//!
//! ## Context
//!
//! With [`Parser::parse`] we get errors that point to the failure but don't explain the reason for
//! the failure:
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::Result;
//! # use winnow::token::take_while;
//! # use winnow::combinator::alt;
//! # use winnow::token::take;
//! # use winnow::combinator::fail;
//! # use winnow::Parser;
//! #
//! # #[derive(Debug, PartialEq, Eq)]
//! # pub struct Hex(usize);
//! #
//! # impl std::str::FromStr for Hex {
//! #     type Err = String;
//! #
//! #     fn from_str(input: &str) -> Result<Self, Self::Err> {
//! #         parse_digits
//! #             .try_map(|(t, v)| match t {
//! #                "0b" => usize::from_str_radix(v, 2),
//! #                "0o" => usize::from_str_radix(v, 8),
//! #                "0d" => usize::from_str_radix(v, 10),
//! #                "0x" => usize::from_str_radix(v, 16),
//! #                _ => unreachable!("`parse_digits` doesn't return `{t}`"),
//! #              })
//! #             .map(Hex)
//! #             .parse(input)
//! #             .map_err(|e| e.to_string())
//! #     }
//! # }
//! #
//! // ...
//!
//! # fn parse_digits<'s>(input: &mut &'s str) -> Result<(&'s str, &'s str)> {
//! #     alt((
//! #         ("0b", parse_bin_digits),
//! #         ("0o", parse_oct_digits),
//! #         ("0d", parse_dec_digits),
//! #         ("0x", parse_hex_digits),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_bin_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='1'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//! fn main() {
//!     let input = "0xZZ";
//!     let error = "\
//! 0xZZ
//!   ^
//! ";
//!     assert_eq!(input.parse::<Hex>().unwrap_err(), error);
//! }
//! ```
//!
//! Back in [`chapter_1`], we glossed over the `Err` variant of [`Result`].  `Result<O>` is
//! actually short for `Result<O, E=ContextError>` where [`ContextError`] is a relatively cheap
//! way of building up reasonable errors for humans.
//!
//! You can use [`Parser::context`] to annotate the error with custom types
//! while unwinding to further clarify the error:
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::Result;
//! # use winnow::token::take_while;
//! # use winnow::combinator::alt;
//! # use winnow::token::take;
//! # use winnow::combinator::fail;
//! # use winnow::Parser;
//! use winnow::error::StrContext;
//! use winnow::error::StrContextValue;
//!
//! #
//! # #[derive(Debug, PartialEq, Eq)]
//! # pub struct Hex(usize);
//! #
//! # impl std::str::FromStr for Hex {
//! #     type Err = String;
//! #
//! #     fn from_str(input: &str) -> Result<Self, Self::Err> {
//! #         parse_digits
//! #             .try_map(|(t, v)| match t {
//! #                "0b" => usize::from_str_radix(v, 2),
//! #                "0o" => usize::from_str_radix(v, 8),
//! #                "0d" => usize::from_str_radix(v, 10),
//! #                "0x" => usize::from_str_radix(v, 16),
//! #                _ => unreachable!("`parse_digits` doesn't return `{t}`"),
//! #              })
//! #             .map(Hex)
//! #             .parse(input)
//! #             .map_err(|e| e.to_string())
//! #     }
//! # }
//! #
//! fn parse_digits<'s>(input: &mut &'s str) -> Result<(&'s str, &'s str)> {
//!     alt((
//!         ("0b", parse_bin_digits)
//!           .context(StrContext::Label("digit"))
//!           .context(StrContext::Expected(StrContextValue::Description("binary"))),
//!         ("0o", parse_oct_digits)
//!           .context(StrContext::Label("digit"))
//!           .context(StrContext::Expected(StrContextValue::Description("octal"))),
//!         ("0d", parse_dec_digits)
//!           .context(StrContext::Label("digit"))
//!           .context(StrContext::Expected(StrContextValue::Description("decimal"))),
//!         ("0x", parse_hex_digits)
//!           .context(StrContext::Label("digit"))
//!           .context(StrContext::Expected(StrContextValue::Description("hexadecimal"))),
//!     )).parse_next(input)
//! }
//!
//! // ...
//!
//! #
//! # fn parse_bin_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='1'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//! fn main() {
//!     let input = "0xZZ";
//!     let error = "\
//! 0xZZ
//!   ^
//! invalid digit
//! expected hexadecimal";
//!     assert_eq!(input.parse::<Hex>().unwrap_err(), error);
//! }
//! ```
//!
//! If you remember back to [`chapter_3`], [`alt`] will only report the last error.
//! So if the parsers fail for any reason, like a bad radix, it will be reported as an invalid
//! hexadecimal value:
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::Result;
//! # use winnow::token::take_while;
//! # use winnow::combinator::alt;
//! # use winnow::token::take;
//! # use winnow::combinator::fail;
//! # use winnow::Parser;
//! # use winnow::error::StrContext;
//! # use winnow::error::StrContextValue;
//! #
//! #
//! # #[derive(Debug, PartialEq, Eq)]
//! # pub struct Hex(usize);
//! #
//! # impl std::str::FromStr for Hex {
//! #     type Err = String;
//! #
//! #     fn from_str(input: &str) -> Result<Self, Self::Err> {
//! #         parse_digits
//! #             .try_map(|(t, v)| match t {
//! #                "0b" => usize::from_str_radix(v, 2),
//! #                "0o" => usize::from_str_radix(v, 8),
//! #                "0d" => usize::from_str_radix(v, 10),
//! #                "0x" => usize::from_str_radix(v, 16),
//! #                _ => unreachable!("`parse_digits` doesn't return `{t}`"),
//! #              })
//! #             .map(Hex)
//! #             .parse(input)
//! #             .map_err(|e| e.to_string())
//! #     }
//! # }
//! #
//! # fn parse_digits<'s>(input: &mut &'s str) -> Result<(&'s str, &'s str)> {
//! #     alt((
//! #         ("0b", parse_bin_digits)
//! #           .context(StrContext::Label("digit"))
//! #           .context(StrContext::Expected(StrContextValue::Description("binary"))),
//! #         ("0o", parse_oct_digits)
//! #           .context(StrContext::Label("digit"))
//! #           .context(StrContext::Expected(StrContextValue::Description("octal"))),
//! #         ("0d", parse_dec_digits)
//! #           .context(StrContext::Label("digit"))
//! #           .context(StrContext::Expected(StrContextValue::Description("decimal"))),
//! #         ("0x", parse_hex_digits)
//! #           .context(StrContext::Label("digit"))
//! #           .context(StrContext::Expected(StrContextValue::Description("hexadecimal"))),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_bin_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='1'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//! fn main() {
//!     let input = "100";
//!     let error = "\
//! 100
//! ^
//! invalid digit
//! expected hexadecimal";
//!     assert_eq!(input.parse::<Hex>().unwrap_err(), error);
//! }
//! ```
//! We can improve this with [`fail`]:
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::Result;
//! # use winnow::token::take_while;
//! # use winnow::combinator::alt;
//! # use winnow::token::take;
//! # use winnow::combinator::fail;
//! # use winnow::Parser;
//! use winnow::error::StrContext;
//! use winnow::error::StrContextValue;
//!
//! #
//! # #[derive(Debug, PartialEq, Eq)]
//! # pub struct Hex(usize);
//! #
//! # impl std::str::FromStr for Hex {
//! #     type Err = String;
//! #
//! #     fn from_str(input: &str) -> Result<Self, Self::Err> {
//! #         parse_digits
//! #             .try_map(|(t, v)| match t {
//! #                "0b" => usize::from_str_radix(v, 2),
//! #                "0o" => usize::from_str_radix(v, 8),
//! #                "0d" => usize::from_str_radix(v, 10),
//! #                "0x" => usize::from_str_radix(v, 16),
//! #                _ => unreachable!("`parse_digits` doesn't return `{t}`"),
//! #              })
//! #             .map(Hex)
//! #             .parse(input)
//! #             .map_err(|e| e.to_string())
//! #     }
//! # }
//! #
//! fn parse_digits<'s>(input: &mut &'s str) -> Result<(&'s str, &'s str)> {
//!     alt((
//!         ("0b", parse_bin_digits)
//!           .context(StrContext::Label("digit"))
//!           .context(StrContext::Expected(StrContextValue::Description("binary"))),
//!         ("0o", parse_oct_digits)
//!           .context(StrContext::Label("digit"))
//!           .context(StrContext::Expected(StrContextValue::Description("octal"))),
//!         ("0d", parse_dec_digits)
//!           .context(StrContext::Label("digit"))
//!           .context(StrContext::Expected(StrContextValue::Description("decimal"))),
//!         ("0x", parse_hex_digits)
//!           .context(StrContext::Label("digit"))
//!           .context(StrContext::Expected(StrContextValue::Description("hexadecimal"))),
//!         fail
//!           .context(StrContext::Label("radix prefix"))
//!           .context(StrContext::Expected(StrContextValue::StringLiteral("0b")))
//!           .context(StrContext::Expected(StrContextValue::StringLiteral("0o")))
//!           .context(StrContext::Expected(StrContextValue::StringLiteral("0d")))
//!           .context(StrContext::Expected(StrContextValue::StringLiteral("0x"))),
//!     )).parse_next(input)
//! }
//!
//! // ...
//!
//! #
//! # fn parse_bin_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='1'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//! fn main() {
//!     let input = "100";
//!     let error = "\
//! 100
//! ^
//! invalid radix prefix
//! expected `0b`, `0o`, `0d`, `0x`";
//!     assert_eq!(input.parse::<Hex>().unwrap_err(), error);
//! }
//! ```
//!
//! ## Error Cuts
//!
//! We still have the issue that we are falling-through when the radix is valid but the digits
//! don't match it:
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::Result;
//! # use winnow::token::take_while;
//! # use winnow::combinator::alt;
//! # use winnow::token::take;
//! # use winnow::combinator::fail;
//! # use winnow::Parser;
//! # use winnow::error::StrContext;
//! # use winnow::error::StrContextValue;
//! #
//! #
//! # #[derive(Debug, PartialEq, Eq)]
//! # pub struct Hex(usize);
//! #
//! # impl std::str::FromStr for Hex {
//! #     type Err = String;
//! #
//! #     fn from_str(input: &str) -> Result<Self, Self::Err> {
//! #         parse_digits
//! #             .try_map(|(t, v)| match t {
//! #                "0b" => usize::from_str_radix(v, 2),
//! #                "0o" => usize::from_str_radix(v, 8),
//! #                "0d" => usize::from_str_radix(v, 10),
//! #                "0x" => usize::from_str_radix(v, 16),
//! #                _ => unreachable!("`parse_digits` doesn't return `{t}`"),
//! #              })
//! #             .map(Hex)
//! #             .parse(input)
//! #             .map_err(|e| e.to_string())
//! #     }
//! # }
//! #
//! # fn parse_digits<'s>(input: &mut &'s str) -> Result<(&'s str, &'s str)> {
//! #     alt((
//! #         ("0b", parse_bin_digits)
//! #           .context(StrContext::Label("digit"))
//! #           .context(StrContext::Expected(StrContextValue::Description("binary"))),
//! #         ("0o", parse_oct_digits)
//! #           .context(StrContext::Label("digit"))
//! #           .context(StrContext::Expected(StrContextValue::Description("octal"))),
//! #         ("0d", parse_dec_digits)
//! #           .context(StrContext::Label("digit"))
//! #           .context(StrContext::Expected(StrContextValue::Description("decimal"))),
//! #         ("0x", parse_hex_digits)
//! #           .context(StrContext::Label("digit"))
//! #           .context(StrContext::Expected(StrContextValue::Description("hexadecimal"))),
//! #         fail
//! #           .context(StrContext::Label("radix prefix"))
//! #           .context(StrContext::Expected(StrContextValue::StringLiteral("0b")))
//! #           .context(StrContext::Expected(StrContextValue::StringLiteral("0o")))
//! #           .context(StrContext::Expected(StrContextValue::StringLiteral("0d")))
//! #           .context(StrContext::Expected(StrContextValue::StringLiteral("0x"))),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_bin_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='1'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//! fn main() {
//!     let input = "0b5";
//!     let error = "\
//! 0b5
//! ^
//! invalid radix prefix
//! expected `0b`, `0o`, `0d`, `0x`";
//!     assert_eq!(input.parse::<Hex>().unwrap_err(), error);
//! }
//! ```
//!
//! Winnow provides an error wrapper, [`ErrMode<ContextError>`], so different failure modes can affect parsing.
//! [`ErrMode`] is an enum with [`Backtrack`] and [`Cut`] variants (ignore [`Incomplete`] as its only
//! relevant for [streaming][_topic::stream]). By default, errors are [`Backtrack`], meaning that
//! other parsing branches will be attempted on failure, like the next case of an [`alt`].  [`Cut`]
//! shortcircuits all other branches, immediately reporting the error.
//!
//! To make [`ErrMode`] more convenient, Winnow provides [`ModalResult`]:
//! ```rust
//! # use winnow::error::ContextError;
//! # use winnow::error::ErrMode;
//! pub type ModalResult<O, E = ContextError> = Result<O, ErrMode<E>>;
//! ```
//!
//! So we can get the correct `context` by changing to [`ModalResult`] and adding [`cut_err`]:
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::token::take_while;
//! # use winnow::combinator::alt;
//! # use winnow::token::take;
//! # use winnow::combinator::fail;
//! # use winnow::Parser;
//! # use winnow::error::StrContext;
//! # use winnow::error::StrContextValue;
//! use winnow::combinator::cut_err;
//!
//! #
//! # #[derive(Debug, PartialEq, Eq)]
//! # pub struct Hex(usize);
//! #
//! # impl std::str::FromStr for Hex {
//! #     type Err = String;
//! #
//! #     fn from_str(input: &str) -> Result<Self, Self::Err> {
//! #         parse_digits
//! #             .try_map(|(t, v)| match t {
//! #                "0b" => usize::from_str_radix(v, 2),
//! #                "0o" => usize::from_str_radix(v, 8),
//! #                "0d" => usize::from_str_radix(v, 10),
//! #                "0x" => usize::from_str_radix(v, 16),
//! #                _ => unreachable!("`parse_digits` doesn't return `{t}`"),
//! #              })
//! #             .map(Hex)
//! #             .parse(input)
//! #             .map_err(|e| e.to_string())
//! #     }
//! # }
//! #
//! fn parse_digits<'s>(input: &mut &'s str) -> ModalResult<(&'s str, &'s str)> {
//!     alt((
//!         ("0b", cut_err(parse_bin_digits))
//!           .context(StrContext::Label("digit"))
//!           .context(StrContext::Expected(StrContextValue::Description("binary"))),
//!         ("0o", cut_err(parse_oct_digits))
//!           .context(StrContext::Label("digit"))
//!           .context(StrContext::Expected(StrContextValue::Description("octal"))),
//!         ("0d", cut_err(parse_dec_digits))
//!           .context(StrContext::Label("digit"))
//!           .context(StrContext::Expected(StrContextValue::Description("decimal"))),
//!         ("0x", cut_err(parse_hex_digits))
//!           .context(StrContext::Label("digit"))
//!           .context(StrContext::Expected(StrContextValue::Description("hexadecimal"))),
//!         fail
//!           .context(StrContext::Label("radix prefix"))
//!           .context(StrContext::Expected(StrContextValue::StringLiteral("0b")))
//!           .context(StrContext::Expected(StrContextValue::StringLiteral("0o")))
//!           .context(StrContext::Expected(StrContextValue::StringLiteral("0d")))
//!           .context(StrContext::Expected(StrContextValue::StringLiteral("0x"))),
//!     )).parse_next(input)
//! }
//!
//! // ...
//!
//! #
//! # fn parse_bin_digits<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='1'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//! fn main() {
//!     let input = "0b5";
//!     let error = "\
//! 0b5
//!   ^
//! invalid digit
//! expected binary";
//!     assert_eq!(input.parse::<Hex>().unwrap_err(), error);
//! }
//! ```
//!
//! ## Error Adaptation and Rendering
//!
//! While Winnow can provide basic rendering of errors, your application can have various demands
//! beyond the basics provided like
//! - Correctly reporting columns with unicode
//! - Conforming to a specific layout
//!
//! For example, to get rustc-like errors with [`annotate-snippets`](https://crates.io/crates/annotate-snippets):
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::token::take_while;
//! # use winnow::combinator::alt;
//! # use winnow::token::take;
//! # use winnow::combinator::fail;
//! # use winnow::Parser;
//! # use winnow::error::ParseError;
//! # use winnow::error::ContextError;
//! # use winnow::error::StrContext;
//! # use winnow::error::StrContextValue;
//! # use winnow::combinator::cut_err;
//! #
//! #
//! #[derive(Debug, PartialEq, Eq)]
//! pub struct Hex(usize);
//!
//! impl std::str::FromStr for Hex {
//!     type Err = HexError;
//!
//!     fn from_str(input: &str) -> Result<Self, Self::Err> {
//!         // ...
//! #         parse_digits
//! #             .try_map(|(t, v)| match t {
//! #                "0b" => usize::from_str_radix(v, 2),
//! #                "0o" => usize::from_str_radix(v, 8),
//! #                "0d" => usize::from_str_radix(v, 10),
//! #                "0x" => usize::from_str_radix(v, 16),
//! #                _ => unreachable!("`parse_digits` doesn't return `{t}`"),
//! #              })
//! #             .map(Hex)
//!             .parse(input)
//!             .map_err(|e| HexError::from_parse(e))
//!     }
//! }
//!
//! #[derive(Debug)]
//! pub struct HexError {
//!     message: String,
//!     // Byte spans are tracked, rather than line and column.
//!     // This makes it easier to operate on programmatically
//!     // and doesn't limit us to one definition for column count
//!     // which can depend on the output medium and application.
//!     span: std::ops::Range<usize>,
//!     input: String,
//! }
//!
//! impl HexError {
//!     // Avoiding `From` so `winnow` types don't become part of our public API
//!     fn from_parse(error: ParseError<&str, ContextError>) -> Self {
//!         // The default renderer for `ContextError` is still used but that can be
//!         // customized as well to better fit your needs.
//!         let message = error.inner().to_string();
//!         let input = (*error.input()).to_owned();
//!         // Assume the error span is only for the first `char`.
//!         let span = error.char_span();
//!         Self {
//!             message,
//!             span,
//!             input,
//!         }
//!     }
//! }
//!
//! impl std::fmt::Display for HexError {
//!     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//!         let message = annotate_snippets::Level::Error.title(&self.message)
//!             .snippet(annotate_snippets::Snippet::source(&self.input)
//!                 .fold(true)
//!                 .annotation(annotate_snippets::Level::Error.span(self.span.clone()))
//!             );
//!         let renderer = annotate_snippets::Renderer::plain();
//!         let rendered = renderer.render(message);
//!         rendered.fmt(f)
//!     }
//! }
//!
//! impl std::error::Error for HexError {}
//!
//! # fn parse_digits<'s>(input: &mut &'s str) -> ModalResult<(&'s str, &'s str)> {
//! #     alt((
//! #         ("0b", cut_err(parse_bin_digits))
//! #           .context(StrContext::Label("digit"))
//! #           .context(StrContext::Expected(StrContextValue::Description("binary"))),
//! #         ("0o", cut_err(parse_oct_digits))
//! #           .context(StrContext::Label("digit"))
//! #           .context(StrContext::Expected(StrContextValue::Description("octal"))),
//! #         ("0d", cut_err(parse_dec_digits))
//! #           .context(StrContext::Label("digit"))
//! #           .context(StrContext::Expected(StrContextValue::Description("decimal"))),
//! #         ("0x", cut_err(parse_hex_digits))
//! #           .context(StrContext::Label("digit"))
//! #           .context(StrContext::Expected(StrContextValue::Description("hexadecimal"))),
//! #         fail
//! #           .context(StrContext::Label("radix prefix"))
//! #           .context(StrContext::Expected(StrContextValue::StringLiteral("0b")))
//! #           .context(StrContext::Expected(StrContextValue::StringLiteral("0o")))
//! #           .context(StrContext::Expected(StrContextValue::StringLiteral("0d")))
//! #           .context(StrContext::Expected(StrContextValue::StringLiteral("0x"))),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_bin_digits<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='1'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//! fn main() {
//!     let input = "0b5";
//!     let error = "\
//! error: invalid digit
//! expected binary
//!   |
//! 1 | 0b5
//!   |   ^
//!   |";
//!     assert_eq!(input.parse::<Hex>().unwrap_err().to_string(), error);
//! }
//! ```
//!
//! To add spans to your parsed data for inclusion in semantic errors, see [`Parser::with_span`].
//!
//! For richer syntactic with spans,
//! consider separating lexing and parsing and annotating your tokens with [`Parser::with_span`].

#![allow(unused_imports)]
use super::chapter_1;
use super::chapter_3;
use crate::combinator::alt;
use crate::combinator::cut_err;
use crate::combinator::fail;
use crate::error::ContextError;
use crate::error::ErrMode;
use crate::error::ErrMode::*;
use crate::ModalResult;
use crate::Parser;
use crate::Result;
use crate::_topic;

pub use super::chapter_6 as previous;
pub use super::chapter_8 as next;
pub use crate::_tutorial as table_of_contents;
