use crate::de::enum_adjacently;
use crate::de::enum_externally;
use crate::de::enum_internally;
use crate::de::enum_untagged;
use crate::de::identifier;
use crate::de::{field_i, FieldWithAliases, Parameters};
use crate::fragment::{Expr, Fragment, Stmts};
use crate::internals::ast::Variant;
use crate::internals::attr;
use crate::private;
use proc_macro2::TokenStream;
use quote::quote;

/// Generates `Deserialize::deserialize` body for an `enum Enum {...}`
pub(super) fn deserialize(
    params: &Parameters,
    variants: &[Variant],
    cattrs: &attr::Container,
) -> Fragment {
    // The variants have already been checked (in ast.rs) that all untagged variants appear at the end
    match variants.iter().position(|var| var.attrs.untagged()) {
        Some(variant_idx) => {
            let (tagged, untagged) = variants.split_at(variant_idx);
            let tagged_frag = Expr(deserialize_homogeneous_enum(params, tagged, cattrs));
            // Ignore any error associated with non-untagged deserialization so that we
            // can fall through to the untagged variants. This may be infallible so we
            // need to provide the error type.
            let first_attempt = quote! {
                if let _serde::#private::Result::<_, __D::Error>::Ok(__ok) = (|| #tagged_frag)() {
                    return _serde::#private::Ok(__ok);
                }
            };
            enum_untagged::deserialize(params, untagged, cattrs, Some(first_attempt))
        }
        None => deserialize_homogeneous_enum(params, variants, cattrs),
    }
}

fn deserialize_homogeneous_enum(
    params: &Parameters,
    variants: &[Variant],
    cattrs: &attr::Container,
) -> Fragment {
    match cattrs.tag() {
        attr::TagType::External => enum_externally::deserialize(params, variants, cattrs),
        attr::TagType::Internal { tag } => {
            enum_internally::deserialize(params, variants, cattrs, tag)
        }
        attr::TagType::Adjacent { tag, content } => {
            enum_adjacently::deserialize(params, variants, cattrs, tag, content)
        }
        attr::TagType::None => enum_untagged::deserialize(params, variants, cattrs, None),
    }
}

pub fn prepare_enum_variant_enum(variants: &[Variant]) -> (TokenStream, Stmts) {
    let deserialized_variants = variants
        .iter()
        .enumerate()
        .filter(|&(_i, variant)| !variant.attrs.skip_deserializing());

    let fallthrough = deserialized_variants
        .clone()
        .find(|(_i, variant)| variant.attrs.other())
        .map(|(i, _variant)| {
            let ignore_variant = field_i(i);
            quote!(_serde::#private::Ok(__Field::#ignore_variant))
        });

    let variants_stmt = {
        let variant_names = deserialized_variants
            .clone()
            .flat_map(|(_i, variant)| variant.attrs.aliases());
        quote! {
            #[doc(hidden)]
            const VARIANTS: &'static [&'static str] = &[ #(#variant_names),* ];
        }
    };

    let deserialized_variants: Vec<_> = deserialized_variants
        .map(|(i, variant)| FieldWithAliases {
            ident: field_i(i),
            aliases: variant.attrs.aliases(),
        })
        .collect();

    let variant_visitor = Stmts(identifier::deserialize_generated(
        &deserialized_variants,
        false, // variant identifiers do not depend on the presence of flatten fields
        true,
        None,
        fallthrough,
    ));

    (variants_stmt, variant_visitor)
}
