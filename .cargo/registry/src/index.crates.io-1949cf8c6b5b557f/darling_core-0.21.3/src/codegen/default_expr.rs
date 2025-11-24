use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens, TokenStreamExt};
use syn::{spanned::Spanned, Ident, Path};

/// This will be in scope during struct initialization after option parsing.
const DEFAULT_STRUCT_NAME: &str = "__default";

/// The fallback value for a field or container.
#[derive(Debug, Clone)]
pub enum DefaultExpression<'a> {
    /// Only valid on fields, `Inherit` indicates that the value should be taken from a pre-constructed
    /// fallback object. The value in the variant is the ident of the field.
    Inherit(&'a Ident),
    Explicit(&'a Path),
    Trait {
        span: Span,
    },
}

impl<'a> DefaultExpression<'a> {
    pub fn as_declaration(&'a self) -> DefaultDeclaration<'a> {
        DefaultDeclaration(self)
    }
}

impl ToTokens for DefaultExpression<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append_all(match *self {
            DefaultExpression::Inherit(ident) => {
                let dsn = Ident::new(DEFAULT_STRUCT_NAME, ::proc_macro2::Span::call_site());
                quote!(#dsn.#ident)
            }
            DefaultExpression::Explicit(path) => {
                // Use quote_spanned to properly set the span of the parentheses
                quote_spanned!(path.span()=>#path())
            }
            DefaultExpression::Trait { span } => {
                quote_spanned!(span=> ::darling::export::Default::default())
            }
        });
    }
}

/// Used only by containers, this wrapper type generates code to declare the fallback instance.
pub struct DefaultDeclaration<'a>(&'a DefaultExpression<'a>);

impl ToTokens for DefaultDeclaration<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = Ident::new(DEFAULT_STRUCT_NAME, ::proc_macro2::Span::call_site());
        let expr = self.0;
        tokens.append_all(quote!(let #name: Self = #expr;));
    }
}
