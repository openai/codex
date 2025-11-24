use quote::ToTokens;
use serde_derive_internals::Ctxt;
use std::collections::HashSet;
use syn::parse::Parser;
use syn::{Attribute, Data, Field, Meta, Variant};

use super::get_meta_items;

// List of keywords that can appear in #[serde(...)]/#[schemars(...)] attributes which we want serde_derive_internals to parse for us.
pub(crate) static SERDE_KEYWORDS: &[&str] = &[
    "rename",
    "rename_all",
    "deny_unknown_fields",
    "tag",
    "content",
    "untagged",
    "default",
    "skip",
    "skip_serializing",
    "skip_serializing_if",
    "skip_deserializing",
    "flatten",
    "remote",
    "transparent",
    // Special case - `bound` is removed from serde attrs, so is only respected when present in schemars attr.
    "bound",
    // Special cases - `with`/`serialize_with` are passed to serde but not copied from schemars attrs to serde attrs.
    // This is because we want to preserve any serde attribute's `serialize_with` value to determine whether the field's
    // default value should be serialized. We also check the `with` value on schemars/serde attrs e.g. to support deriving
    // JsonSchema on remote types, but we parse that ourselves rather than using serde_derive_internals.
    "serialize_with",
    "with",
];

// If a struct/variant/field has any #[schemars] attributes, then create copies of them
// as #[serde] attributes so that serde_derive_internals will parse them for us.
pub fn process_serde_attrs(input: &mut syn::DeriveInput) -> syn::Result<()> {
    let ctxt = Ctxt::new();
    process_attrs(&ctxt, &mut input.attrs);
    match input.data {
        Data::Struct(ref mut s) => process_serde_field_attrs(&ctxt, s.fields.iter_mut()),
        Data::Enum(ref mut e) => process_serde_variant_attrs(&ctxt, e.variants.iter_mut()),
        Data::Union(ref mut u) => process_serde_field_attrs(&ctxt, u.fields.named.iter_mut()),
    };

    ctxt.check()
}

fn process_serde_variant_attrs<'a>(ctxt: &Ctxt, variants: impl Iterator<Item = &'a mut Variant>) {
    for v in variants {
        process_attrs(ctxt, &mut v.attrs);
        process_serde_field_attrs(ctxt, v.fields.iter_mut());
    }
}

fn process_serde_field_attrs<'a>(ctxt: &Ctxt, fields: impl Iterator<Item = &'a mut Field>) {
    for f in fields {
        process_attrs(ctxt, &mut f.attrs);
    }
}

fn process_attrs(ctxt: &Ctxt, attrs: &mut Vec<Attribute>) {
    // Remove #[serde(...)] attributes (some may be re-added later)
    let (serde_attrs, other_attrs): (Vec<_>, Vec<_>) =
        attrs.drain(..).partition(|at| at.path().is_ident("serde"));
    *attrs = other_attrs;

    // Copy appropriate #[schemars(...)] attributes to #[serde(...)] attributes
    let (mut serde_meta, mut schemars_meta_names): (Vec<_>, HashSet<_>) =
        get_meta_items(attrs, "schemars", ctxt, false)
            .into_iter()
            .filter_map(|meta| {
                let keyword = get_meta_ident(&meta)?;
                if SERDE_KEYWORDS.contains(&keyword.as_ref()) && !keyword.ends_with("with") {
                    Some((meta, keyword))
                } else {
                    None
                }
            })
            .unzip();

    if schemars_meta_names.contains("skip") {
        schemars_meta_names.insert("skip_serializing".to_string());
        schemars_meta_names.insert("skip_deserializing".to_string());
    }

    // Re-add #[serde(...)] attributes that weren't overridden by #[schemars(...)] attributes
    for meta in get_meta_items(&serde_attrs, "serde", ctxt, false) {
        if let Some(i) = get_meta_ident(&meta) {
            if !schemars_meta_names.contains(&i)
                && SERDE_KEYWORDS.contains(&i.as_ref())
                && i != "bound"
            {
                serde_meta.push(meta);
            }
        }
    }

    if !serde_meta.is_empty() {
        let new_serde_attr = quote! {
            #[serde(#(#serde_meta),*)]
        };

        let parser = Attribute::parse_outer;
        match parser.parse2(new_serde_attr) {
            Ok(ref mut parsed) => attrs.append(parsed),
            Err(e) => ctxt.error_spanned_by(to_tokens(attrs), e),
        }
    }
}

fn to_tokens(attrs: &[Attribute]) -> impl ToTokens {
    let mut tokens = proc_macro2::TokenStream::new();
    for attr in attrs {
        attr.to_tokens(&mut tokens);
    }
    tokens
}

fn get_meta_ident(meta: &Meta) -> Option<String> {
    meta.path().get_ident().map(|i| i.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use syn::DeriveInput;

    #[test]
    fn test_process_serde_attrs() {
        let mut input: DeriveInput = parse_quote! {
            #[serde(rename(serialize = "ser_name"), rename_all = "camelCase")]
            #[serde(default, unknown_word)]
            #[schemars(rename = "overriden", another_unknown_word)]
            #[misc]
            struct MyStruct {
                /// blah blah blah
                #[serde(skip_serializing_if = "some_fn", bound = "removed")]
                field1: i32,
                #[serde(serialize_with = "se", deserialize_with = "de")]
                #[schemars(with = "with", bound = "bound")]
                field2: i32,
                #[schemars(skip)]
                #[serde(skip_serializing)]
                field3: i32,
            }
        };
        let expected: DeriveInput = parse_quote! {
            #[schemars(rename = "overriden", another_unknown_word)]
            #[misc]
            #[serde(rename = "overriden", rename_all = "camelCase", default)]
            struct MyStruct {
                #[doc = r" blah blah blah"]
                #[serde(skip_serializing_if = "some_fn")]
                field1: i32,
                #[schemars(with = "with", bound = "bound")]
                #[serde(bound = "bound", serialize_with = "se")]
                field2: i32,
                #[schemars(skip)]
                #[serde(skip)]
                field3: i32,
            }
        };

        if let Err(e) = process_serde_attrs(&mut input) {
            panic!("process_serde_attrs returned error: {}", e)
        };

        assert_eq!(input, expected);
    }
}
