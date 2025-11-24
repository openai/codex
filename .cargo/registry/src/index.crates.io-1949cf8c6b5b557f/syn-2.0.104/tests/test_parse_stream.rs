#![allow(clippy::items_after_statements, clippy::let_underscore_untyped)]

use proc_macro2::{Delimiter, Group, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::quote;
use syn::ext::IdentExt as _;
use syn::parse::discouraged::AnyDelimiter;
use syn::parse::{ParseStream, Parser as _, Result};
use syn::{parenthesized, token, Ident, Lifetime, Token};

#[test]
fn test_peek_punct() {
    let tokens = quote!(+= + =);

    fn assert(input: ParseStream) -> Result<()> {
        assert!(input.peek(Token![+]));
        assert!(input.peek(Token![+=]));

        let _: Token![+] = input.parse()?;

        assert!(input.peek(Token![=]));
        assert!(!input.peek(Token![==]));
        assert!(!input.peek(Token![+]));

        let _: Token![=] = input.parse()?;

        assert!(input.peek(Token![+]));
        assert!(!input.peek(Token![+=]));

        let _: Token![+] = input.parse()?;
        let _: Token![=] = input.parse()?;
        Ok(())
    }

    assert.parse2(tokens).unwrap();
}

#[test]
fn test_peek_lifetime() {
    // 'static ;
    let tokens = TokenStream::from_iter([
        TokenTree::Punct(Punct::new('\'', Spacing::Joint)),
        TokenTree::Ident(Ident::new("static", Span::call_site())),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);

    fn assert(input: ParseStream) -> Result<()> {
        assert!(input.peek(Lifetime));
        assert!(input.peek2(Token![;]));
        assert!(!input.peek2(Token![static]));

        let _: Lifetime = input.parse()?;

        assert!(input.peek(Token![;]));

        let _: Token![;] = input.parse()?;
        Ok(())
    }

    assert.parse2(tokens).unwrap();
}

#[test]
fn test_peek_not_lifetime() {
    // ' static
    let tokens = TokenStream::from_iter([
        TokenTree::Punct(Punct::new('\'', Spacing::Alone)),
        TokenTree::Ident(Ident::new("static", Span::call_site())),
    ]);

    fn assert(input: ParseStream) -> Result<()> {
        assert!(!input.peek(Lifetime));
        assert!(input.parse::<Option<Punct>>()?.is_none());

        let _: TokenTree = input.parse()?;

        assert!(input.peek(Token![static]));

        let _: Token![static] = input.parse()?;
        Ok(())
    }

    assert.parse2(tokens).unwrap();
}

#[test]
fn test_peek_ident() {
    let tokens = quote!(static var);

    fn assert(input: ParseStream) -> Result<()> {
        assert!(!input.peek(Ident));
        assert!(input.peek(Ident::peek_any));
        assert!(input.peek(Token![static]));

        let _: Token![static] = input.parse()?;

        assert!(input.peek(Ident));
        assert!(input.peek(Ident::peek_any));

        let _: Ident = input.parse()?;
        Ok(())
    }

    assert.parse2(tokens).unwrap();
}

#[test]
fn test_peek_groups() {
    // pub ( :: ) «∅ ! = ∅» static
    let tokens = TokenStream::from_iter([
        TokenTree::Ident(Ident::new("pub", Span::call_site())),
        TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            TokenStream::from_iter([
                TokenTree::Punct(Punct::new(':', Spacing::Joint)),
                TokenTree::Punct(Punct::new(':', Spacing::Alone)),
            ]),
        )),
        TokenTree::Group(Group::new(
            Delimiter::None,
            TokenStream::from_iter([
                TokenTree::Punct(Punct::new('!', Spacing::Alone)),
                TokenTree::Punct(Punct::new('=', Spacing::Alone)),
            ]),
        )),
        TokenTree::Ident(Ident::new("static", Span::call_site())),
    ]);

    fn assert(input: ParseStream) -> Result<()> {
        assert!(input.peek2(token::Paren));
        assert!(input.peek3(token::Group));
        assert!(input.peek3(Token![!]));

        let _: Token![pub] = input.parse()?;

        assert!(input.peek(token::Paren));
        assert!(!input.peek(Token![::]));
        assert!(!input.peek2(Token![::]));
        assert!(input.peek2(Token![!]));
        assert!(input.peek2(token::Group));
        assert!(input.peek3(Token![=]));
        assert!(!input.peek3(Token![static]));

        let content;
        parenthesized!(content in input);

        assert!(content.peek(Token![::]));
        assert!(content.peek2(Token![:]));
        assert!(!content.peek3(token::Group));
        assert!(!content.peek3(Token![!]));

        assert!(input.peek(token::Group));
        assert!(input.peek(Token![!]));

        let _: Token![::] = content.parse()?;

        assert!(input.peek(token::Group));
        assert!(input.peek(Token![!]));
        assert!(input.peek2(Token![=]));
        assert!(input.peek3(Token![static]));
        assert!(!input.peek2(Token![static]));

        let implicit = input.fork();
        let explicit = input.fork();

        let _: Token![!] = implicit.parse()?;
        assert!(implicit.peek(Token![=]));
        assert!(implicit.peek2(Token![static]));
        let _: Token![=] = implicit.parse()?;
        assert!(implicit.peek(Token![static]));

        let (delimiter, _span, grouped) = explicit.parse_any_delimiter()?;
        assert_eq!(delimiter, Delimiter::None);
        assert!(grouped.peek(Token![!]));
        assert!(grouped.peek2(Token![=]));
        assert!(!grouped.peek3(Token![static]));
        let _: Token![!] = grouped.parse()?;
        assert!(grouped.peek(Token![=]));
        assert!(!grouped.peek2(Token![static]));
        let _: Token![=] = grouped.parse()?;
        assert!(!grouped.peek(Token![static]));

        let _: TokenStream = input.parse()?;
        Ok(())
    }

    assert.parse2(tokens).unwrap();
}
