use proc_macro2::{Ident, Span, TokenStream};
use quote::TokenStreamExt;

pub const GENERATOR: ConstIdent = ConstIdent("generator");
pub const SCHEMA: ConstIdent = ConstIdent("schema");
pub const STRUCT_DEFAULT: ConstIdent = ConstIdent("struct_default");

pub struct ConstIdent(&'static str);

impl quote::ToTokens for ConstIdent {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = Ident::new(self.0, Span::call_site());
        tokens.append(ident);
    }
}
