use std::fmt;
use std::hash::{Hash, Hasher};

use proc_macro2::{Span, TokenStream};
use quote::ToTokens;
use syn::{Ident, Meta};

use crate::{FromMeta, Result};

/// A wrapper for an `Ident` which also keeps the value as a string.
///
/// This struct can be used to perform string comparisons and operations.
#[derive(Clone, PartialOrd, Ord)]
pub struct IdentString {
    ident: Ident,
    string: String,
}

impl IdentString {
    /// Create a new `IdentString`.
    pub fn new(ident: Ident) -> Self {
        IdentString {
            string: ident.to_string(),
            ident,
        }
    }

    /// Get the ident as a `proc_macro2::Ident`.
    pub fn as_ident(&self) -> &Ident {
        &self.ident
    }

    /// Get the ident as a string.
    pub fn as_str(&self) -> &str {
        &self.string
    }

    /// Get the location of this `Ident` in source.
    pub fn span(&self) -> Span {
        self.ident.span()
    }

    /// Apply some transform to the ident's string representation.
    ///
    /// # Panics
    /// This will panic if the transform produces an invalid ident.
    pub fn map<F, S>(self, map_fn: F) -> Self
    where
        F: FnOnce(String) -> S,
        S: AsRef<str>,
    {
        let span = self.span();
        let string = map_fn(self.string);
        Ident::new(string.as_ref(), span).into()
    }
}

impl AsRef<Ident> for IdentString {
    fn as_ref(&self) -> &Ident {
        self.as_ident()
    }
}

impl AsRef<str> for IdentString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<Ident> for IdentString {
    fn from(ident: Ident) -> Self {
        IdentString::new(ident)
    }
}

impl From<IdentString> for Ident {
    fn from(v: IdentString) -> Ident {
        v.ident
    }
}

impl From<IdentString> for String {
    fn from(v: IdentString) -> String {
        v.string
    }
}

impl Eq for IdentString {}

impl PartialEq for IdentString {
    fn eq(&self, rhs: &Self) -> bool {
        self.ident == rhs.ident
    }
}

impl PartialEq<String> for IdentString {
    fn eq(&self, rhs: &String) -> bool {
        self.as_str() == rhs
    }
}

impl PartialEq<&str> for IdentString {
    fn eq(&self, rhs: &&str) -> bool {
        self.as_str() == *rhs
    }
}

impl Hash for IdentString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ident.hash(state);
    }
}

impl ToTokens for IdentString {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.ident.to_tokens(tokens);
    }
}

impl fmt::Debug for IdentString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.ident)
    }
}

impl fmt::Display for IdentString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.ident)
    }
}

impl FromMeta for IdentString {
    fn from_meta(item: &Meta) -> Result<Self> {
        Ident::from_meta(item).map(IdentString::from)
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::IdentString;

    #[test]
    fn convert() {
        let i_str = IdentString::new(parse_quote!(t));
        assert_eq!(i_str.as_str(), "t");
    }

    #[test]
    fn map_transform() {
        let i = IdentString::new(parse_quote!(my));
        let after = i.map(|v| format!("var_{}", v));
        assert_eq!(after, "var_my");
        assert_eq!(after, String::from("var_my"));
    }
}
