use proc_macro2::{Ident, Span, TokenStream};
use quote::ToTokens;
use std::cmp::Ordering;
use std::fmt::{self, Display};
use std::hash::{Hash, Hasher};
use syn::ext::IdentExt as _;
use syn::parse::{Parse, ParseStream, Result};
use syn::Index;

#[derive(Clone)]
#[repr(transparent)]
pub struct IdentUnraw(Ident);

impl IdentUnraw {
    pub fn new(ident: Ident) -> Self {
        IdentUnraw(ident)
    }

    pub fn to_local(&self) -> Ident {
        let unraw = self.0.unraw();
        let repr = unraw.to_string();
        if syn::parse_str::<Ident>(&repr).is_err() {
            if let "_" | "super" | "self" | "Self" | "crate" = repr.as_str() {
                // Some identifiers are never allowed to appear as raw, like r#self and r#_.
            } else {
                return Ident::new_raw(&repr, Span::call_site());
            }
        }
        unraw
    }

    pub fn set_span(&mut self, span: Span) {
        self.0.set_span(span);
    }
}

impl Display for IdentUnraw {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0.unraw(), formatter)
    }
}

impl Eq for IdentUnraw {}

impl PartialEq for IdentUnraw {
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(&self.0.unraw(), &other.0.unraw())
    }
}

impl PartialEq<str> for IdentUnraw {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl Ord for IdentUnraw {
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(&self.0.unraw(), &other.0.unraw())
    }
}

impl PartialOrd for IdentUnraw {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Self::cmp(self, other))
    }
}

impl Parse for IdentUnraw {
    fn parse(input: ParseStream) -> Result<Self> {
        input.call(Ident::parse_any).map(IdentUnraw::new)
    }
}

impl ToTokens for IdentUnraw {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.unraw().to_tokens(tokens);
    }
}

#[derive(Clone)]
pub enum MemberUnraw {
    Named(IdentUnraw),
    Unnamed(Index),
}

impl MemberUnraw {
    pub fn span(&self) -> Span {
        match self {
            MemberUnraw::Named(ident) => ident.0.span(),
            MemberUnraw::Unnamed(index) => index.span,
        }
    }
}

impl Display for MemberUnraw {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MemberUnraw::Named(this) => Display::fmt(this, formatter),
            MemberUnraw::Unnamed(this) => Display::fmt(&this.index, formatter),
        }
    }
}

impl Eq for MemberUnraw {}

impl PartialEq for MemberUnraw {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MemberUnraw::Named(this), MemberUnraw::Named(other)) => this == other,
            (MemberUnraw::Unnamed(this), MemberUnraw::Unnamed(other)) => this == other,
            _ => false,
        }
    }
}

impl PartialEq<str> for MemberUnraw {
    fn eq(&self, other: &str) -> bool {
        match self {
            MemberUnraw::Named(this) => this == other,
            MemberUnraw::Unnamed(_) => false,
        }
    }
}

impl Hash for MemberUnraw {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        match self {
            MemberUnraw::Named(ident) => ident.0.unraw().hash(hasher),
            MemberUnraw::Unnamed(index) => index.hash(hasher),
        }
    }
}

impl ToTokens for MemberUnraw {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            MemberUnraw::Named(ident) => ident.to_local().to_tokens(tokens),
            MemberUnraw::Unnamed(index) => index.to_tokens(tokens),
        }
    }
}
