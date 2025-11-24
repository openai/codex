use quote::ToTokens;
use syn::{parse::Parse, Meta, MetaList, MetaNameValue, Path};

// An extended copy of `syn::Meta` with an additional `Not` variant
#[derive(Clone)]
pub enum CustomMeta {
    Path(Path),
    List(MetaList),
    NameValue(MetaNameValue),
    Not(Token![!], Path),
}

impl ToTokens for CustomMeta {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            CustomMeta::Not(not, path) => {
                not.to_tokens(tokens);
                path.to_tokens(tokens);
            }
            CustomMeta::Path(meta) => meta.to_tokens(tokens),
            CustomMeta::List(meta) => meta.to_tokens(tokens),
            CustomMeta::NameValue(meta) => meta.to_tokens(tokens),
        }
    }
}

impl From<Meta> for CustomMeta {
    fn from(value: Meta) -> Self {
        match value {
            Meta::Path(meta) => Self::Path(meta),
            Meta::List(meta) => Self::List(meta),
            Meta::NameValue(meta) => Self::NameValue(meta),
        }
    }
}

impl Parse for CustomMeta {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(if input.peek(Token![!]) {
            Self::Not(input.parse()?, input.parse()?)
        } else {
            Meta::parse(input)?.into()
        })
    }
}

impl CustomMeta {
    pub fn path(&self) -> &Path {
        match self {
            CustomMeta::Not(_not, path) => path,
            CustomMeta::Path(path) => path,
            CustomMeta::List(meta) => &meta.path,
            CustomMeta::NameValue(meta) => &meta.path,
        }
    }
}
