use quote::ToTokens;
use serde_derive_internals::Ctxt;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use syn::parse::Parser;
use syn::{Attribute, Data, Field, Variant};

use super::{get_meta_items, CustomMeta};

// List of keywords that can appear in #[serde(...)]/#[schemars(...)] attributes which we want
// serde_derive_internals to parse for us.
pub(crate) static SERDE_KEYWORDS: &[&str] = &[
    "rename",
    "rename_all",
    "rename_all_fields",
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
    "into",
    "from",
    "try_from",
    // Special case - `bound` is removed from serde attrs, so is only respected when present in
    // schemars attr.
    "bound",
    // Special cases - `with`/`serialize_with` are passed to serde but not copied from schemars
    // attrs to serde attrs. This is because we want to preserve any serde attribute's
    // `serialize_with` value to determine whether the field's default value should be
    // serialized. We also check the `with` value on schemars/serde attrs e.g. to support deriving
    // JsonSchema on remote types, but we parse that ourselves rather than using
    // serde_derive_internals.
    "serialize_with",
    "with",
];

pub(crate) static SCHEMARS_KEYWORDS_PARSED_BY_SERDE: &[&str] =
    // exclude "serialize_with" and "with"
    SERDE_KEYWORDS.split_at(SERDE_KEYWORDS.len() - 2).0;

// If a struct/variant/field has any #[schemars] attributes, then create copies of them
// as #[serde] attributes so that serde_derive_internals will parse them for us.
pub fn process_serde_attrs(input: &mut syn::DeriveInput) -> syn::Result<()> {
    let ctxt = Ctxt::new();
    process_attrs(&ctxt, &mut input.attrs);
    match &mut input.data {
        Data::Struct(s) => process_serde_field_attrs(&ctxt, s.fields.iter_mut()),
        Data::Enum(e) => process_serde_variant_attrs(&ctxt, e.variants.iter_mut()),
        Data::Union(u) => process_serde_field_attrs(&ctxt, u.fields.named.iter_mut()),
    }

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

    let mut effective_serde_meta = Vec::new();
    let mut unset_meta = BTreeMap::new();
    let mut serde_meta_names = BTreeSet::new();
    let mut schemars_meta_names = BTreeSet::new();

    // Copy appropriate #[schemars(...)] attributes to #[serde(...)] attributes
    for meta in get_meta_items(attrs, "schemars", ctxt) {
        let Some(keyword) = get_meta_ident(&meta) else {
            continue;
        };

        if matches!(meta, CustomMeta::Not(..)) {
            match unset_meta.entry(keyword) {
                Entry::Occupied(o) => {
                    ctxt.error_spanned_by(
                        meta,
                        format_args!("duplicate schemars attribute item `!{}`", o.key()),
                    );
                }
                Entry::Vacant(v) => {
                    v.insert(meta);
                }
            }
        } else if SCHEMARS_KEYWORDS_PARSED_BY_SERDE.contains(&keyword.as_ref()) {
            schemars_meta_names.insert(keyword);
            effective_serde_meta.push(meta);
        }
    }

    for (keyword, meta) in &unset_meta {
        if schemars_meta_names.contains(keyword) {
            ctxt.error_spanned_by(
                meta,
                format_args!("schemars attribute cannot contain both `{keyword}` and `!{keyword}`"),
            );
        }
    }

    if schemars_meta_names.contains("skip") {
        schemars_meta_names.insert("skip_serializing".to_string());
        schemars_meta_names.insert("skip_deserializing".to_string());
    }

    // Re-add #[serde(...)] attributes that weren't overridden by #[schemars(...)] attributes
    for meta in get_meta_items(&serde_attrs, "serde", ctxt) {
        let Some(keyword) = get_meta_ident(&meta) else {
            continue;
        };

        if !schemars_meta_names.contains(&keyword)
            && !unset_meta.contains_key(&keyword)
            && SERDE_KEYWORDS.contains(&keyword.as_ref())
            && keyword != "bound"
        {
            effective_serde_meta.push(meta);
        }

        serde_meta_names.insert(keyword);
    }

    for (keyword, meta) in &unset_meta {
        if !serde_meta_names.contains(keyword) {
            ctxt.error_spanned_by(
                meta,
                format_args!(
                    "useless `!{keyword}` - no serde attribute containing `{keyword}` is present"
                ),
            );
        }
    }

    if !effective_serde_meta.is_empty() {
        let new_serde_attr = quote! {
            #[serde(#(#effective_serde_meta),*)]
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

fn get_meta_ident(meta: &CustomMeta) -> Option<String> {
    meta.path().get_ident().map(std::string::ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use syn::DeriveInput;

    #[test]
    fn test_process_serde_attrs() {
        let mut input: DeriveInput = parse_quote! {
            #[serde(rename(serialize = "ser_name"), rename_all = "camelCase", from = "T")]
            #[serde(default, unknown_word)]
            #[schemars(rename = "overriden", another_unknown_word, !from)]
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
            #[schemars(rename = "overriden", another_unknown_word, !from)]
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
            panic!("process_serde_attrs returned error: {e}")
        }

        assert_eq!(input, expected);
    }
}
