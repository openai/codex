use super::metadata::{InnerVariantExt, InnerVariantMeta};
use super::occurrence_error;
use syn::{Field, LitStr};

pub trait HasInnerVariantProperties {
    fn get_variant_inner_properties(&self) -> syn::Result<StrumInnerVariantProperties>;
}

#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct StrumInnerVariantProperties {
    pub default_with: Option<LitStr>,
}

impl HasInnerVariantProperties for Field {
    fn get_variant_inner_properties(&self) -> syn::Result<StrumInnerVariantProperties> {
        let mut output = StrumInnerVariantProperties { default_with: None };

        let mut default_with_kw = None;
        for meta in self.get_named_metadata()? {
            match meta {
                InnerVariantMeta::DefaultWith { kw, value } => {
                    if let Some(fst_kw) = default_with_kw {
                        return Err(occurrence_error(fst_kw, kw, "default_with"));
                    }
                    default_with_kw = Some(kw);
                    output.default_with = Some(value);
                }
            }
        }

        Ok(output)
    }
}
