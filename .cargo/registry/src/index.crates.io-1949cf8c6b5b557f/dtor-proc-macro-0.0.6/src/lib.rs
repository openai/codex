#![doc = include_str!("../README.md")]

use std::iter::FromIterator;

use proc_macro::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};

#[allow(missing_docs)]
#[proc_macro_attribute]
pub fn dtor(attribute: TokenStream, item: TokenStream) -> TokenStream {
    generate("dtor", "dtor", attribute, item)
}

#[allow(missing_docs)]
#[proc_macro_attribute]
pub fn __dtor_from_ctor(attribute: TokenStream, item: TokenStream) -> TokenStream {
    generate("ctor", "dtor", attribute, item)
}

/// Generates the equivalent of this Rust code as a TokenStream:
///
/// ```nocompile
/// ::ctor::__support::ctor_parse!(#[ctor] fn foo() { ... });
/// ::dtor::__support::dtor_parse!(#[dtor] fn foo() { ... });
/// ```
#[allow(unknown_lints, tail_expr_drop_order)]
fn generate(
    macro_crate: &str,
    macro_type: &str,
    attribute: TokenStream,
    item: TokenStream,
) -> TokenStream {
    let mut inner = TokenStream::new();

    // Search for crate_path in attributes
    let mut crate_path = None;
    let mut tokens = attribute.clone().into_iter().peekable();

    while let Some(token) = tokens.next() {
        if let TokenTree::Ident(ident) = &token {
            if ident.to_string() == "crate_path" {
                // Look for =
                if let Some(TokenTree::Punct(punct)) = tokens.next() {
                    if punct.as_char() == '=' {
                        // Collect tokens until comma or end
                        let mut path = TokenStream::new();
                        while let Some(token) = tokens.peek() {
                            match token {
                                TokenTree::Punct(p) if p.as_char() == ',' => {
                                    tokens.next();
                                    break;
                                }
                                _ => {
                                    path.extend(std::iter::once(tokens.next().unwrap()));
                                }
                            }
                        }
                        crate_path = Some(path);
                        break;
                    }
                }
            }
        }
    }

    if attribute.is_empty() {
        // #[ctor]
        inner.extend([
            TokenTree::Punct(Punct::new('#', Spacing::Alone)),
            TokenTree::Group(Group::new(
                Delimiter::Bracket,
                TokenStream::from_iter([TokenTree::Ident(Ident::new(
                    macro_type,
                    Span::call_site(),
                ))]),
            )),
        ]);
    } else {
        inner.extend([
            TokenTree::Punct(Punct::new('#', Spacing::Alone)),
            TokenTree::Group(Group::new(
                Delimiter::Bracket,
                TokenStream::from_iter([
                    TokenTree::Ident(Ident::new(macro_type, Span::call_site())),
                    TokenTree::Group(Group::new(Delimiter::Parenthesis, attribute)),
                ]),
            )),
        ]);
    }

    inner.extend(item);

    let mut invoke = crate_path.unwrap_or_else(|| {
        TokenStream::from_iter([
            TokenTree::Punct(Punct::new(':', Spacing::Joint)),
            TokenTree::Punct(Punct::new(':', Spacing::Alone)),
            TokenTree::Ident(Ident::new(macro_crate, Span::call_site())),
        ])
    });

    invoke.extend([
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("__support", Span::call_site())),
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new(
            &format!("{macro_type}_parse"),
            Span::call_site(),
        )),
        TokenTree::Punct(Punct::new('!', Spacing::Alone)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, inner)),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);

    invoke
}
