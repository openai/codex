use quote::{quote, quote_spanned, ToTokens, TokenStreamExt};
use syn::spanned::Spanned;

use crate::options::{ForwardAttrsFilter, ForwardedField};

#[derive(Default)]
pub struct ForwardAttrs<'a> {
    pub filter: Option<&'a ForwardAttrsFilter>,
    pub field: Option<&'a ForwardedField>,
}

impl ForwardAttrs<'_> {
    /// Check if this will forward any attributes; this requires both that
    /// there be a filter which can match some attributes and a field to receive them.
    pub fn will_forward_any(&self) -> bool {
        if let Some(filter) = self.filter {
            !filter.is_empty() && self.field.is_some()
        } else {
            false
        }
    }

    /// Get the field declarations to support attribute forwarding
    pub fn as_declaration(&self) -> Option<Declaration<'_>> {
        self.field.map(Declaration)
    }

    /// Get the match arms for attribute matching
    pub fn as_match_arms(&self) -> MatchArms<'_> {
        MatchArms(self)
    }

    /// Get the statement that will try to transform forwarded attributes into
    /// the result expected by the receiver field.
    pub fn as_value_populator(&self) -> Option<ValuePopulator<'_>> {
        self.field.map(ValuePopulator)
    }

    /// Get the field initializer for use when building the deriving struct.
    pub fn as_initializer(&self) -> Option<Initializer<'_>> {
        self.field.map(Initializer)
    }
}

pub struct Declaration<'a>(pub &'a ForwardedField);

impl ToTokens for Declaration<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.0.ident;
        tokens.append_all(quote! {
            let mut __fwd_attrs: ::darling::export::Vec<::darling::export::syn::Attribute> = vec![];
            let mut #ident: ::darling::export::Option<_> = None;
        });
    }
}

pub struct ValuePopulator<'a>(pub &'a ForwardedField);

impl ToTokens for ValuePopulator<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ForwardedField { ident, with } = self.0;
        let initializer_expr = match with {
            Some(with) => quote_spanned!(with.span()=> __errors.handle(#with(__fwd_attrs))),
            None => quote!(::darling::export::Some(__fwd_attrs)),
        };
        tokens.append_all(quote!(#ident = #initializer_expr;));
    }
}

pub struct Initializer<'a>(pub &'a ForwardedField);

impl ToTokens for Initializer<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.0.ident;
        tokens.append_all(quote!(#ident: #ident.expect("Errors were already checked"),));
    }
}

pub struct MatchArms<'a>(&'a ForwardAttrs<'a>);

impl ToTokens for MatchArms<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        if !self.0.will_forward_any() {
            tokens.append_all(quote!(_ => continue));
            return;
        }

        let push_command = quote!(__fwd_attrs.push(__attr.clone()));

        tokens.append_all(
            match self
                .0
                .filter
                .expect("Can only forward attributes if filter is defined")
            {
                ForwardAttrsFilter::All => quote!(_ => #push_command),
                ForwardAttrsFilter::Only(idents) => {
                    let names = idents.to_strings();
                    quote! {
                        #(#names)|* => #push_command,
                        _ => continue,
                    }
                }
            },
        );
    }
}
