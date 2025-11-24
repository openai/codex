use anyhow::{bail, Error};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::Meta;

use crate::field::{set_bool, set_option, tag_attr, word_attr, Label};

#[derive(Clone)]
pub struct Field {
    pub label: Label,
    pub tag: u32,
}

impl Field {
    pub fn new(attrs: &[Meta], inferred_tag: Option<u32>) -> Result<Option<Field>, Error> {
        let mut group = false;
        let mut label = None;
        let mut tag = None;
        let mut boxed = false;

        let mut unknown_attrs = Vec::new();

        for attr in attrs {
            if word_attr("group", attr) {
                set_bool(&mut group, "duplicate group attributes")?;
            } else if word_attr("boxed", attr) {
                set_bool(&mut boxed, "duplicate boxed attributes")?;
            } else if let Some(t) = tag_attr(attr)? {
                set_option(&mut tag, t, "duplicate tag attributes")?;
            } else if let Some(l) = Label::from_attr(attr) {
                set_option(&mut label, l, "duplicate label attributes")?;
            } else {
                unknown_attrs.push(attr);
            }
        }

        if !group {
            return Ok(None);
        }

        if !unknown_attrs.is_empty() {
            bail!(
                "unknown attribute(s) for group field: #[prost({})]",
                quote!(#(#unknown_attrs),*)
            );
        }

        let tag = match tag.or(inferred_tag) {
            Some(tag) => tag,
            None => bail!("group field is missing a tag attribute"),
        };

        Ok(Some(Field {
            label: label.unwrap_or(Label::Optional),
            tag,
        }))
    }

    pub fn new_oneof(attrs: &[Meta]) -> Result<Option<Field>, Error> {
        if let Some(mut field) = Field::new(attrs, None)? {
            if let Some(attr) = attrs.iter().find(|attr| Label::from_attr(attr).is_some()) {
                bail!(
                    "invalid attribute for oneof field: {}",
                    attr.path().into_token_stream()
                );
            }
            field.label = Label::Required;
            Ok(Some(field))
        } else {
            Ok(None)
        }
    }

    pub fn encode(&self, ident: TokenStream) -> TokenStream {
        let tag = self.tag;
        match self.label {
            Label::Optional => quote! {
                if let Some(ref msg) = #ident {
                    ::prost::encoding::group::encode(#tag, msg, buf);
                }
            },
            Label::Required => quote! {
                ::prost::encoding::group::encode(#tag, &#ident, buf);
            },
            Label::Repeated => quote! {
                for msg in &#ident {
                    ::prost::encoding::group::encode(#tag, msg, buf);
                }
            },
        }
    }

    pub fn merge(&self, ident: TokenStream) -> TokenStream {
        match self.label {
            Label::Optional => quote! {
                ::prost::encoding::group::merge(
                    tag,
                    wire_type,
                    #ident.get_or_insert_with(::core::default::Default::default),
                    buf,
                    ctx,
                )
            },
            Label::Required => quote! {
                ::prost::encoding::group::merge(tag, wire_type, #ident, buf, ctx)
            },
            Label::Repeated => quote! {
                ::prost::encoding::group::merge_repeated(tag, wire_type, #ident, buf, ctx)
            },
        }
    }

    pub fn encoded_len(&self, ident: TokenStream) -> TokenStream {
        let tag = self.tag;
        match self.label {
            Label::Optional => quote! {
                #ident.as_ref().map_or(0, |msg| ::prost::encoding::group::encoded_len(#tag, msg))
            },
            Label::Required => quote! {
                ::prost::encoding::group::encoded_len(#tag, &#ident)
            },
            Label::Repeated => quote! {
                ::prost::encoding::group::encoded_len_repeated(#tag, &#ident)
            },
        }
    }

    pub fn clear(&self, ident: TokenStream) -> TokenStream {
        match self.label {
            Label::Optional => quote!(#ident = ::core::option::Option::None),
            Label::Required => quote!(#ident.clear()),
            Label::Repeated => quote!(#ident.clear()),
        }
    }
}
