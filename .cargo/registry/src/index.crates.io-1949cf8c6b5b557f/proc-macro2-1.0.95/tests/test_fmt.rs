#![allow(clippy::from_iter_instead_of_collect)]

use proc_macro2::{Delimiter, Group, Ident, Span, TokenStream, TokenTree};
use std::iter;

#[test]
fn test_fmt_group() {
    let ident = Ident::new("x", Span::call_site());
    let inner = TokenStream::from_iter(iter::once(TokenTree::Ident(ident)));
    let parens_empty = Group::new(Delimiter::Parenthesis, TokenStream::new());
    let parens_nonempty = Group::new(Delimiter::Parenthesis, inner.clone());
    let brackets_empty = Group::new(Delimiter::Bracket, TokenStream::new());
    let brackets_nonempty = Group::new(Delimiter::Bracket, inner.clone());
    let braces_empty = Group::new(Delimiter::Brace, TokenStream::new());
    let braces_nonempty = Group::new(Delimiter::Brace, inner.clone());
    let none_empty = Group::new(Delimiter::None, TokenStream::new());
    let none_nonempty = Group::new(Delimiter::None, inner);

    // Matches libproc_macro.
    assert_eq!("()", parens_empty.to_string());
    assert_eq!("(x)", parens_nonempty.to_string());
    assert_eq!("[]", brackets_empty.to_string());
    assert_eq!("[x]", brackets_nonempty.to_string());
    assert_eq!("{ }", braces_empty.to_string());
    assert_eq!("{ x }", braces_nonempty.to_string());
    assert_eq!("", none_empty.to_string());
    assert_eq!("x", none_nonempty.to_string());
}
