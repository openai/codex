use syn::{Attribute, Expr, Fields, Ident, Result, Type, Variant};

use super::{parse_assign_expr, Attr, Serde};
use crate::{
    attr::{parse_assign_from_str, parse_assign_inflection, parse_assign_str, Inflection},
    utils::parse_attrs,
};

#[derive(Default)]
pub struct VariantAttr {
    pub type_as: Option<Type>,
    pub type_override: Option<String>,
    pub rename: Option<Expr>,
    pub rename_all: Option<Inflection>,
    pub inline: bool,
    pub skip: bool,
    pub untagged: bool,
}

impl VariantAttr {
    pub fn from_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut result = parse_attrs::<Self>(attrs)?;
        if cfg!(feature = "serde-compat") && !result.skip {
            let serde_attr = crate::utils::parse_serde_attrs::<VariantAttr>(attrs);
            result = result.merge(serde_attr.0);
        }
        Ok(result)
    }
}

impl Attr for VariantAttr {
    type Item = Variant;

    fn merge(self, other: Self) -> Self {
        Self {
            type_as: self.type_as.or(other.type_as),
            type_override: self.type_override.or(other.type_override),
            rename: self.rename.or(other.rename),
            rename_all: self.rename_all.or(other.rename_all),
            inline: self.inline || other.inline,
            skip: self.skip || other.skip,
            untagged: self.untagged || other.untagged,
        }
    }

    fn assert_validity(&self, item: &Self::Item) -> Result<()> {
        if self.type_as.is_some() {
            if self.type_override.is_some() {
                syn_err_spanned!(
                    item;
                    "`as` is not compatible with `type`"
                )
            }

            if self.rename_all.is_some() {
                syn_err_spanned!(
                    item;
                    "`as` is not compatible with `rename_all`"
                )
            }
        }

        if self.type_override.is_some() {
            if self.rename_all.is_some() {
                syn_err_spanned!(
                    item;
                    "`type` is not compatible with `rename_all`"
                )
            }

            if self.inline {
                syn_err_spanned!(
                    item;
                    "`type` is not compatible with `inline`"
                )
            }
        }

        if !matches!(item.fields, Fields::Named(_)) && self.rename_all.is_some() {
            syn_err_spanned!(
                item;
                "`rename_all` is not applicable to unit or tuple variants"
            )
        }

        Ok(())
    }
}

impl_parse! {
    VariantAttr(input, out) {
        "as" => out.type_as = Some(parse_assign_from_str(input)?),
        "type" => out.type_override = Some(parse_assign_str(input)?),
        "rename" => out.rename = Some(parse_assign_expr(input)?),
        "rename_all" => out.rename_all = Some(parse_assign_inflection(input)?),
        "inline" => out.inline = true,
        "skip" => out.skip = true,
        "untagged" => out.untagged = true,
    }
}

impl_parse! {
    Serde<VariantAttr>(input, out) {
        "rename" => out.0.rename = Some(parse_assign_expr(input)?),
        "rename_all" => out.0.rename_all = Some(parse_assign_inflection(input)?),
        "skip" => out.0.skip = true,
        "untagged" => out.0.untagged = true,
    }
}
