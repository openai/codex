use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::{Token, TypeParamBound};

pub type Supertraits = Punctuated<TypeParamBound, Token![+]>;

pub enum InferredBound {
    Send,
    Sync,
}

pub fn has_bound(supertraits: &Supertraits, bound: &InferredBound) -> bool {
    for supertrait in supertraits {
        if let TypeParamBound::Trait(supertrait) = supertrait {
            if supertrait.path.is_ident(bound)
                || supertrait.path.segments.len() == 3
                    && (supertrait.path.segments[0].ident == "std"
                        || supertrait.path.segments[0].ident == "core")
                    && supertrait.path.segments[1].ident == "marker"
                    && supertrait.path.segments[2].ident == *bound
            {
                return true;
            }
        }
    }
    false
}

impl InferredBound {
    fn as_str(&self) -> &str {
        match self {
            InferredBound::Send => "Send",
            InferredBound::Sync => "Sync",
        }
    }
}

impl ToTokens for InferredBound {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = Ident::new(self.as_str(), Span::call_site());
        quote!(::core::marker::#ident).to_tokens(tokens);
    }
}

impl PartialEq<InferredBound> for Ident {
    fn eq(&self, bound: &InferredBound) -> bool {
        self == bound.as_str()
    }
}
