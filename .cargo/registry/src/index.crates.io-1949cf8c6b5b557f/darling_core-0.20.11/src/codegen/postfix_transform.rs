use quote::{quote, ToTokens, TokenStreamExt};
use syn::{Ident, Path};

/// A method invocation applied to a value.
///
/// This is used for `map` and `and_then` transforms in derivations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostfixTransform {
    pub(crate) transformer: Ident,
    pub(crate) function: Path,
}

impl PostfixTransform {
    pub fn new(transformer: Ident, function: Path) -> Self {
        Self {
            transformer,
            function,
        }
    }
}

impl ToTokens for PostfixTransform {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            transformer,
            function,
        } = self;
        tokens.append_all(quote!(.#transformer(#function)))
    }
}
