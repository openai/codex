//! [![github]](https://github.com/dtolnay/paste)&ensp;[![crates-io]](https://crates.io/crates/paste)&ensp;[![docs-rs]](https://docs.rs/paste)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! <br>
//!
//! The nightly-only [`concat_idents!`] macro in the Rust standard library is
//! notoriously underpowered in that its concatenated identifiers can only refer to
//! existing items, they can never be used to define something new.
//!
//! [`concat_idents!`]: https://doc.rust-lang.org/std/macro.concat_idents.html
//!
//! This crate provides a flexible way to paste together identifiers in a macro,
//! including using pasted identifiers to define new items.
//!
//! This approach works with any Rust compiler 1.31+.
//!
//! <br>
//!
//! # Pasting identifiers
//!
//! Within the `paste!` macro, identifiers inside `[<`...`>]` are pasted
//! together to form a single identifier.
//!
//! ```
//! use paste::paste;
//!
//! paste! {
//!     // Defines a const called `QRST`.
//!     const [<Q R S T>]: &str = "success!";
//! }
//!
//! fn main() {
//!     assert_eq!(
//!         paste! { [<Q R S T>].len() },
//!         8,
//!     );
//! }
//! ```
//!
//! <br><br>
//!
//! # More elaborate example
//!
//! The next example shows a macro that generates accessor methods for some
//! struct fields. It demonstrates how you might find it useful to bundle a
//! paste invocation inside of a macro\_rules macro.
//!
//! ```
//! use paste::paste;
//!
//! macro_rules! make_a_struct_and_getters {
//!     ($name:ident { $($field:ident),* }) => {
//!         // Define a struct. This expands to:
//!         //
//!         //     pub struct S {
//!         //         a: String,
//!         //         b: String,
//!         //         c: String,
//!         //     }
//!         pub struct $name {
//!             $(
//!                 $field: String,
//!             )*
//!         }
//!
//!         // Build an impl block with getters. This expands to:
//!         //
//!         //     impl S {
//!         //         pub fn get_a(&self) -> &str { &self.a }
//!         //         pub fn get_b(&self) -> &str { &self.b }
//!         //         pub fn get_c(&self) -> &str { &self.c }
//!         //     }
//!         paste! {
//!             impl $name {
//!                 $(
//!                     pub fn [<get_ $field>](&self) -> &str {
//!                         &self.$field
//!                     }
//!                 )*
//!             }
//!         }
//!     }
//! }
//!
//! make_a_struct_and_getters!(S { a, b, c });
//!
//! fn call_some_getters(s: &S) -> bool {
//!     s.get_a() == s.get_b() && s.get_c().is_empty()
//! }
//! #
//! # fn main() {}
//! ```
//!
//! <br><br>
//!
//! # Case conversion
//!
//! Use `$var:lower` or `$var:upper` in the segment list to convert an
//! interpolated segment to lower- or uppercase as part of the paste. For
//! example, `[<ld_ $reg:lower _expr>]` would paste to `ld_bc_expr` if invoked
//! with $reg=`Bc`.
//!
//! Use `$var:snake` to convert CamelCase input to snake\_case.
//! Use `$var:camel` to convert snake\_case to CamelCase.
//! These compose, so for example `$var:snake:upper` would give you SCREAMING\_CASE.
//!
//! The precise Unicode conversions are as defined by [`str::to_lowercase`] and
//! [`str::to_uppercase`].
//!
//! [`str::to_lowercase`]: https://doc.rust-lang.org/std/primitive.str.html#method.to_lowercase
//! [`str::to_uppercase`]: https://doc.rust-lang.org/std/primitive.str.html#method.to_uppercase
//!
//! <br>
//!
//! # Pasting documentation strings
//!
//! Within the `paste!` macro, arguments to a #\[doc ...\] attribute are
//! implicitly concatenated together to form a coherent documentation string.
//!
//! ```
//! use paste::paste;
//!
//! macro_rules! method_new {
//!     ($ret:ident) => {
//!         paste! {
//!             #[doc = "Create a new `" $ret "` object."]
//!             pub fn new() -> $ret { todo!() }
//!         }
//!     };
//! }
//!
//! pub struct Paste {}
//!
//! method_new!(Paste);  // expands to #[doc = "Create a new `Paste` object"]
//! ```

#![doc(html_root_url = "https://docs.rs/paste/1.0.15")]
#![allow(
    clippy::derive_partial_eq_without_eq,
    clippy::doc_markdown,
    clippy::match_same_arms,
    clippy::module_name_repetitions,
    clippy::needless_doctest_main,
    clippy::too_many_lines
)]

extern crate proc_macro;

mod attr;
mod error;
mod segment;

use crate::attr::expand_attr;
use crate::error::{Error, Result};
use crate::segment::Segment;
use proc_macro::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};
use std::char;
use std::iter;
use std::panic;

#[proc_macro]
pub fn paste(input: TokenStream) -> TokenStream {
    let mut contains_paste = false;
    let flatten_single_interpolation = true;
    match expand(
        input.clone(),
        &mut contains_paste,
        flatten_single_interpolation,
    ) {
        Ok(expanded) => {
            if contains_paste {
                expanded
            } else {
                input
            }
        }
        Err(err) => err.to_compile_error(),
    }
}

#[doc(hidden)]
#[proc_macro]
pub fn item(input: TokenStream) -> TokenStream {
    paste(input)
}

#[doc(hidden)]
#[proc_macro]
pub fn expr(input: TokenStream) -> TokenStream {
    paste(input)
}

fn expand(
    input: TokenStream,
    contains_paste: &mut bool,
    flatten_single_interpolation: bool,
) -> Result<TokenStream> {
    let mut expanded = TokenStream::new();
    let mut lookbehind = Lookbehind::Other;
    let mut prev_none_group = None::<Group>;
    let mut tokens = input.into_iter().peekable();
    loop {
        let token = tokens.next();
        if let Some(group) = prev_none_group.take() {
            if match (&token, tokens.peek()) {
                (Some(TokenTree::Punct(fst)), Some(TokenTree::Punct(snd))) => {
                    fst.as_char() == ':' && snd.as_char() == ':' && fst.spacing() == Spacing::Joint
                }
                _ => false,
            } {
                expanded.extend(group.stream());
                *contains_paste = true;
            } else {
                expanded.extend(iter::once(TokenTree::Group(group)));
            }
        }
        match token {
            Some(TokenTree::Group(group)) => {
                let delimiter = group.delimiter();
                let content = group.stream();
                let span = group.span();
                if delimiter == Delimiter::Bracket && is_paste_operation(&content) {
                    let segments = parse_bracket_as_segments(content, span)?;
                    let pasted = segment::paste(&segments)?;
                    let tokens = pasted_to_tokens(pasted, span)?;
                    expanded.extend(tokens);
                    *contains_paste = true;
                } else if flatten_single_interpolation
                    && delimiter == Delimiter::None
                    && is_single_interpolation_group(&content)
                {
                    expanded.extend(content);
                    *contains_paste = true;
                } else {
                    let mut group_contains_paste = false;
                    let is_attribute = delimiter == Delimiter::Bracket
                        && (lookbehind == Lookbehind::Pound || lookbehind == Lookbehind::PoundBang);
                    let mut nested = expand(
                        content,
                        &mut group_contains_paste,
                        flatten_single_interpolation && !is_attribute,
                    )?;
                    if is_attribute {
                        nested = expand_attr(nested, span, &mut group_contains_paste)?;
                    }
                    let group = if group_contains_paste {
                        let mut group = Group::new(delimiter, nested);
                        group.set_span(span);
                        *contains_paste = true;
                        group
                    } else {
                        group.clone()
                    };
                    if delimiter != Delimiter::None {
                        expanded.extend(iter::once(TokenTree::Group(group)));
                    } else if lookbehind == Lookbehind::DoubleColon {
                        expanded.extend(group.stream());
                        *contains_paste = true;
                    } else {
                        prev_none_group = Some(group);
                    }
                }
                lookbehind = Lookbehind::Other;
            }
            Some(TokenTree::Punct(punct)) => {
                lookbehind = match punct.as_char() {
                    ':' if lookbehind == Lookbehind::JointColon => Lookbehind::DoubleColon,
                    ':' if punct.spacing() == Spacing::Joint => Lookbehind::JointColon,
                    '#' => Lookbehind::Pound,
                    '!' if lookbehind == Lookbehind::Pound => Lookbehind::PoundBang,
                    _ => Lookbehind::Other,
                };
                expanded.extend(iter::once(TokenTree::Punct(punct)));
            }
            Some(other) => {
                lookbehind = Lookbehind::Other;
                expanded.extend(iter::once(other));
            }
            None => return Ok(expanded),
        }
    }
}

#[derive(PartialEq)]
enum Lookbehind {
    JointColon,
    DoubleColon,
    Pound,
    PoundBang,
    Other,
}

// https://github.com/dtolnay/paste/issues/26
fn is_single_interpolation_group(input: &TokenStream) -> bool {
    #[derive(PartialEq)]
    enum State {
        Init,
        Ident,
        Literal,
        Apostrophe,
        Lifetime,
        Colon1,
        Colon2,
    }

    let mut state = State::Init;
    for tt in input.clone() {
        state = match (state, &tt) {
            (State::Init, TokenTree::Ident(_)) => State::Ident,
            (State::Init, TokenTree::Literal(_)) => State::Literal,
            (State::Init, TokenTree::Punct(punct)) if punct.as_char() == '\'' => State::Apostrophe,
            (State::Apostrophe, TokenTree::Ident(_)) => State::Lifetime,
            (State::Ident, TokenTree::Punct(punct))
                if punct.as_char() == ':' && punct.spacing() == Spacing::Joint =>
            {
                State::Colon1
            }
            (State::Colon1, TokenTree::Punct(punct))
                if punct.as_char() == ':' && punct.spacing() == Spacing::Alone =>
            {
                State::Colon2
            }
            (State::Colon2, TokenTree::Ident(_)) => State::Ident,
            _ => return false,
        };
    }

    state == State::Ident || state == State::Literal || state == State::Lifetime
}

fn is_paste_operation(input: &TokenStream) -> bool {
    let mut tokens = input.clone().into_iter();

    match &tokens.next() {
        Some(TokenTree::Punct(punct)) if punct.as_char() == '<' => {}
        _ => return false,
    }

    let mut has_token = false;
    loop {
        match &tokens.next() {
            Some(TokenTree::Punct(punct)) if punct.as_char() == '>' => {
                return has_token && tokens.next().is_none();
            }
            Some(_) => has_token = true,
            None => return false,
        }
    }
}

fn parse_bracket_as_segments(input: TokenStream, scope: Span) -> Result<Vec<Segment>> {
    let mut tokens = input.into_iter().peekable();

    match &tokens.next() {
        Some(TokenTree::Punct(punct)) if punct.as_char() == '<' => {}
        Some(wrong) => return Err(Error::new(wrong.span(), "expected `<`")),
        None => return Err(Error::new(scope, "expected `[< ... >]`")),
    }

    let mut segments = segment::parse(&mut tokens)?;

    match &tokens.next() {
        Some(TokenTree::Punct(punct)) if punct.as_char() == '>' => {}
        Some(wrong) => return Err(Error::new(wrong.span(), "expected `>`")),
        None => return Err(Error::new(scope, "expected `[< ... >]`")),
    }

    if let Some(unexpected) = tokens.next() {
        return Err(Error::new(
            unexpected.span(),
            "unexpected input, expected `[< ... >]`",
        ));
    }

    for segment in &mut segments {
        if let Segment::String(string) = segment {
            if string.value.starts_with("'\\u{") {
                let hex = &string.value[4..string.value.len() - 2];
                if let Ok(unsigned) = u32::from_str_radix(hex, 16) {
                    if let Some(ch) = char::from_u32(unsigned) {
                        string.value.clear();
                        string.value.push(ch);
                        continue;
                    }
                }
            }
            if string.value.contains(&['#', '\\', '.', '+'][..])
                || string.value.starts_with("b'")
                || string.value.starts_with("b\"")
                || string.value.starts_with("br\"")
            {
                return Err(Error::new(string.span, "unsupported literal"));
            }
            let mut range = 0..string.value.len();
            if string.value.starts_with("r\"") {
                range.start += 2;
                range.end -= 1;
            } else if string.value.starts_with(&['"', '\''][..]) {
                range.start += 1;
                range.end -= 1;
            }
            string.value = string.value[range].replace('-', "_");
        }
    }

    Ok(segments)
}

fn pasted_to_tokens(mut pasted: String, span: Span) -> Result<TokenStream> {
    let mut tokens = TokenStream::new();

    #[cfg(not(no_literal_fromstr))]
    {
        use proc_macro::{LexError, Literal};
        use std::str::FromStr;

        if pasted.starts_with(|ch: char| ch.is_ascii_digit()) {
            let literal = match panic::catch_unwind(|| Literal::from_str(&pasted)) {
                Ok(Ok(literal)) => TokenTree::Literal(literal),
                Ok(Err(LexError { .. })) | Err(_) => {
                    return Err(Error::new(
                        span,
                        &format!("`{:?}` is not a valid literal", pasted),
                    ));
                }
            };
            tokens.extend(iter::once(literal));
            return Ok(tokens);
        }
    }

    if pasted.starts_with('\'') {
        let mut apostrophe = TokenTree::Punct(Punct::new('\'', Spacing::Joint));
        apostrophe.set_span(span);
        tokens.extend(iter::once(apostrophe));
        pasted.remove(0);
    }

    let ident = match panic::catch_unwind(|| Ident::new(&pasted, span)) {
        Ok(ident) => TokenTree::Ident(ident),
        Err(_) => {
            return Err(Error::new(
                span,
                &format!("`{:?}` is not a valid identifier", pasted),
            ));
        }
    };

    tokens.extend(iter::once(ident));
    Ok(tokens)
}
