use proc_macro2::{Spacing, Span, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use syn::Ident;

/// Analog to Option<TokenStream>, except when put into the quote!
/// macro, `MaybeVoid::Void` will produce `()`
#[derive(Clone)]
pub enum MaybeVoid {
    Some(TokenStream),
    Void,
}

impl Default for MaybeVoid {
    fn default() -> MaybeVoid {
        MaybeVoid::Void
    }
}

impl MaybeVoid {
    pub fn replace(&mut self, stream: TokenStream) -> MaybeVoid {
        std::mem::replace(self, MaybeVoid::Some(stream))
    }

    pub fn take(&mut self) -> MaybeVoid {
        std::mem::replace(self, MaybeVoid::Void)
    }
}

impl ToTokens for MaybeVoid {
    fn to_tokens(&self, out: &mut TokenStream) {
        match self {
            MaybeVoid::Some(stream) => out.extend(stream.clone()),
            MaybeVoid::Void => out.extend(quote!(())),
        }
    }

    fn to_token_stream(&self) -> TokenStream {
        match self {
            MaybeVoid::Some(stream) => stream.clone(),
            MaybeVoid::Void => quote!(()),
        }
    }

    fn into_token_stream(self) -> TokenStream {
        match self {
            MaybeVoid::Some(stream) => stream,
            MaybeVoid::Void => quote!(()),
        }
    }
}

pub fn is_punct(tt: &TokenTree, expect: char) -> bool {
    match tt {
        TokenTree::Punct(punct)
            if punct.as_char() == expect && punct.spacing() == Spacing::Alone =>
        {
            true
        }
        _ => false,
    }
}

/// If supplied `tt` is a punct matching a char, returns `None`, else returns `tt`
pub fn expect_punct(tt: Option<TokenTree>, expect: char) -> Option<TokenTree> {
    tt.filter(|tt| !is_punct(&tt, expect))
}

pub trait ToIdent {
    fn to_ident(&self) -> Ident;
}

impl ToIdent for str {
    fn to_ident(&self) -> Ident {
        Ident::new(self, Span::call_site())
    }
}
