use proc_macro2::token_stream::IntoIter as TokenIter;
use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::quote;

use crate::util::{expect_punct, is_punct};

pub enum NestedValue {
    /// `name = ...`
    Assign(TokenStream),
    /// `name(...)`
    Group(TokenStream),
    /// `name ident = ...`
    KeywordAssign(Ident, TokenStream),
}

pub enum Nested {
    /// Unnamed nested attribute, such as a string,
    /// callback closure, or a lone ident/path
    ///
    /// Note: a lone ident will be Named with no value instead
    Unnamed(TokenStream),
    /// Named: name ...
    Named(Ident, NestedValue),
    /// Unexpected token,
    Unexpected(TokenStream),
}

pub struct AttributeParser {
    inner: TokenIter,
}

pub struct Empty;

impl From<Empty> for TokenStream {
    fn from(_: Empty) -> TokenStream {
        TokenStream::new()
    }
}

impl AttributeParser {
    pub fn new(stream: TokenStream) -> Self {
        AttributeParser {
            inner: stream.into_iter(),
        }
    }

    pub fn parsed<T>(&mut self) -> Option<syn::Result<T>>
    where
        T: syn::parse::Parse,
    {
        let tokens = self.collect_tail(TokenStream::new());

        if tokens.is_empty() {
            return None;
        }

        Some(syn::parse2(tokens))
    }

    fn next_tt(&mut self) -> Option<TokenTree> {
        expect_punct(self.inner.next(), ',')
    }

    fn collect_tail<T>(&mut self, first: T) -> TokenStream
    where
        T: Into<TokenStream>,
    {
        let mut out = first.into();

        while let Some(tt) = self.next_tt() {
            out.extend(Some(tt));
        }

        out
    }

    fn parse_unnamed(&mut self, first: Ident, next: TokenTree) -> Nested {
        let mut out = TokenStream::from(TokenTree::Ident(first));

        out.extend(self.collect_tail(next));

        Nested::Unnamed(out.into_iter().collect())
    }

    fn parse_assign(&mut self, name: Ident) -> Nested {
        let value = self.collect_tail(Empty);

        Nested::Named(name, NestedValue::Assign(value))
    }

    fn parse_group(&mut self, name: Ident, group: TokenStream) -> Nested {
        Nested::Named(name, NestedValue::Group(group))
    }

    fn parse_keyword(&mut self, keyword: Ident, name: Ident) -> Nested {
        let error = expect_punct(self.next_tt(), '=');

        match error {
            Some(error) => {
                let error = self.collect_tail(error);

                Nested::Unexpected(error)
            }
            None => {
                let value = self.collect_tail(Empty);

                Nested::Named(keyword, NestedValue::KeywordAssign(name, value))
            }
        }
    }
}

impl Iterator for AttributeParser {
    type Item = Nested;

    fn next(&mut self) -> Option<Nested> {
        let first = self.inner.next()?;

        let name = match first {
            TokenTree::Ident(ident) => ident,
            tt => {
                let stream = self.collect_tail(tt);

                return Some(Nested::Unnamed(stream.into_iter().collect()));
            }
        };

        match self.next_tt() {
            Some(tt) if is_punct(&tt, '=') => Some(self.parse_assign(name)),
            Some(TokenTree::Group(group)) => Some(self.parse_group(name, group.stream())),
            Some(TokenTree::Ident(next)) => Some(self.parse_keyword(name, next)),
            Some(next) => Some(self.parse_unnamed(name, next)),
            None => Some(Nested::Unnamed(quote!(#name))),
        }
    }
}
