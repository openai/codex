use crate::buffer::Cursor;
use crate::error::{self, Error};
use crate::sealed::lookahead::Sealed;
use crate::span::IntoSpans;
use crate::token::Token;
use proc_macro2::{Delimiter, Span};
use std::cell::RefCell;

/// Support for checking the next token in a stream to decide how to parse.
///
/// An important advantage over [`ParseStream::peek`] is that here we
/// automatically construct an appropriate error message based on the token
/// alternatives that get peeked. If you are producing your own error message,
/// go ahead and use `ParseStream::peek` instead.
///
/// Use [`ParseStream::lookahead1`] to construct this object.
///
/// [`ParseStream::peek`]: crate::parse::ParseBuffer::peek
/// [`ParseStream::lookahead1`]: crate::parse::ParseBuffer::lookahead1
///
/// Consuming tokens from the source stream after constructing a lookahead
/// object does not also advance the lookahead object.
///
/// # Example
///
/// ```
/// use syn::{ConstParam, Ident, Lifetime, LifetimeDef, Result, Token, TypeParam};
/// use syn::parse::{Parse, ParseStream};
///
/// // A generic parameter, a single one of the comma-separated elements inside
/// // angle brackets in:
/// //
/// //     fn f<T: Clone, 'a, 'b: 'a, const N: usize>() { ... }
/// //
/// // On invalid input, lookahead gives us a reasonable error message.
/// //
/// //     error: expected one of: identifier, lifetime, `const`
/// //       |
/// //     5 |     fn f<!Sized>() {}
/// //       |          ^
/// enum GenericParam {
///     Type(TypeParam),
///     Lifetime(LifetimeDef),
///     Const(ConstParam),
/// }
///
/// impl Parse for GenericParam {
///     fn parse(input: ParseStream) -> Result<Self> {
///         let lookahead = input.lookahead1();
///         if lookahead.peek(Ident) {
///             input.parse().map(GenericParam::Type)
///         } else if lookahead.peek(Lifetime) {
///             input.parse().map(GenericParam::Lifetime)
///         } else if lookahead.peek(Token![const]) {
///             input.parse().map(GenericParam::Const)
///         } else {
///             Err(lookahead.error())
///         }
///     }
/// }
/// ```
pub struct Lookahead1<'a> {
    scope: Span,
    cursor: Cursor<'a>,
    comparisons: RefCell<Vec<&'static str>>,
}

pub fn new(scope: Span, cursor: Cursor) -> Lookahead1 {
    Lookahead1 {
        scope,
        cursor,
        comparisons: RefCell::new(Vec::new()),
    }
}

fn peek_impl(
    lookahead: &Lookahead1,
    peek: fn(Cursor) -> bool,
    display: fn() -> &'static str,
) -> bool {
    if peek(lookahead.cursor) {
        return true;
    }
    lookahead.comparisons.borrow_mut().push(display());
    false
}

impl<'a> Lookahead1<'a> {
    /// Looks at the next token in the parse stream to determine whether it
    /// matches the requested type of token.
    ///
    /// # Syntax
    ///
    /// Note that this method does not use turbofish syntax. Pass the peek type
    /// inside of parentheses.
    ///
    /// - `input.peek(Token![struct])`
    /// - `input.peek(Token![==])`
    /// - `input.peek(Ident)`&emsp;*(does not accept keywords)*
    /// - `input.peek(Ident::peek_any)`
    /// - `input.peek(Lifetime)`
    /// - `input.peek(token::Brace)`
    pub fn peek<T: Peek>(&self, token: T) -> bool {
        let _ = token;
        peek_impl(self, T::Token::peek, T::Token::display)
    }

    /// Triggers an error at the current position of the parse stream.
    ///
    /// The error message will identify all of the expected token types that
    /// have been peeked against this lookahead instance.
    pub fn error(self) -> Error {
        let comparisons = self.comparisons.borrow();
        match comparisons.len() {
            0 => {
                if self.cursor.eof() {
                    Error::new(self.scope, "unexpected end of input")
                } else {
                    Error::new(self.cursor.span(), "unexpected token")
                }
            }
            1 => {
                let message = format!("expected {}", comparisons[0]);
                error::new_at(self.scope, self.cursor, message)
            }
            2 => {
                let message = format!("expected {} or {}", comparisons[0], comparisons[1]);
                error::new_at(self.scope, self.cursor, message)
            }
            _ => {
                let join = comparisons.join(", ");
                let message = format!("expected one of: {}", join);
                error::new_at(self.scope, self.cursor, message)
            }
        }
    }
}

/// Types that can be parsed by looking at just one token.
///
/// Use [`ParseStream::peek`] to peek one of these types in a parse stream
/// without consuming it from the stream.
///
/// This trait is sealed and cannot be implemented for types outside of Syn.
///
/// [`ParseStream::peek`]: crate::parse::ParseBuffer::peek
pub trait Peek: Sealed {
    // Not public API.
    #[doc(hidden)]
    type Token: Token;
}

impl<F: Copy + FnOnce(TokenMarker) -> T, T: Token> Peek for F {
    type Token = T;
}

pub enum TokenMarker {}

impl<S> IntoSpans<S> for TokenMarker {
    fn into_spans(self) -> S {
        match self {}
    }
}

pub fn is_delimiter(cursor: Cursor, delimiter: Delimiter) -> bool {
    cursor.group(delimiter).is_some()
}

impl<F: Copy + FnOnce(TokenMarker) -> T, T: Token> Sealed for F {}
