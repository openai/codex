use proc_macro2::TokenStream;
use quote::{ToTokens, TokenStreamExt};
use syn::parse::{Parse, ParseStream, Result};
use syn::{Attribute, Signature, Token, Visibility};

pub struct VerbatimFn {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub defaultness: Option<Token![default]>,
    pub sig: Signature,
    pub semi_token: Token![;],
}

impl Parse for VerbatimFn {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(VerbatimFn {
            attrs: input.call(Attribute::parse_outer)?,
            vis: input.parse()?,
            defaultness: input.parse()?,
            sig: input.parse()?,
            semi_token: input.parse()?,
        })
    }
}

impl ToTokens for VerbatimFn {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append_all(&self.attrs);
        self.vis.to_tokens(tokens);
        self.defaultness.to_tokens(tokens);
        self.sig.to_tokens(tokens);
        self.semi_token.to_tokens(tokens);
    }
}
