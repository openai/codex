#![allow(clippy::non_ascii_literal)]

use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, TokenStream, TokenTree};
use std::panic;
use syn::parse::discouraged::Speculative as _;
use syn::parse::{Parse, ParseStream, Parser, Result};
use syn::{parenthesized, Token};

#[test]
#[should_panic(expected = "fork was not derived from the advancing parse stream")]
fn smuggled_speculative_cursor_between_sources() {
    struct BreakRules;
    impl Parse for BreakRules {
        fn parse(input1: ParseStream) -> Result<Self> {
            let nested = |input2: ParseStream| {
                input1.advance_to(input2);
                Ok(Self)
            };
            nested.parse_str("")
        }
    }

    syn::parse_str::<BreakRules>("").unwrap();
}

#[test]
#[should_panic(expected = "fork was not derived from the advancing parse stream")]
fn smuggled_speculative_cursor_between_brackets() {
    struct BreakRules;
    impl Parse for BreakRules {
        fn parse(input: ParseStream) -> Result<Self> {
            let a;
            let b;
            parenthesized!(a in input);
            parenthesized!(b in input);
            a.advance_to(&b);
            Ok(Self)
        }
    }

    syn::parse_str::<BreakRules>("()()").unwrap();
}

#[test]
#[should_panic(expected = "fork was not derived from the advancing parse stream")]
fn smuggled_speculative_cursor_into_brackets() {
    struct BreakRules;
    impl Parse for BreakRules {
        fn parse(input: ParseStream) -> Result<Self> {
            let a;
            parenthesized!(a in input);
            input.advance_to(&a);
            Ok(Self)
        }
    }

    syn::parse_str::<BreakRules>("()").unwrap();
}

#[test]
fn trailing_empty_none_group() {
    fn parse(input: ParseStream) -> Result<()> {
        input.parse::<Token![+]>()?;

        let content;
        parenthesized!(content in input);
        content.parse::<Token![+]>()?;

        Ok(())
    }

    // `+ ( + «∅ ∅» ) «∅ «∅ ∅» ∅»`
    let tokens = TokenStream::from_iter([
        TokenTree::Punct(Punct::new('+', Spacing::Alone)),
        TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            TokenStream::from_iter([
                TokenTree::Punct(Punct::new('+', Spacing::Alone)),
                TokenTree::Group(Group::new(Delimiter::None, TokenStream::new())),
            ]),
        )),
        TokenTree::Group(Group::new(Delimiter::None, TokenStream::new())),
        TokenTree::Group(Group::new(
            Delimiter::None,
            TokenStream::from_iter([TokenTree::Group(Group::new(
                Delimiter::None,
                TokenStream::new(),
            ))]),
        )),
    ]);

    parse.parse2(tokens).unwrap();
}

#[test]
fn test_unwind_safe() {
    fn parse(input: ParseStream) -> Result<Ident> {
        let thread_result = panic::catch_unwind(|| input.parse());
        thread_result.unwrap()
    }

    parse.parse_str("throw").unwrap();
}
