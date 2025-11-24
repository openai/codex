use std::collections::HashMap;

use syn::{parse_quote, Attribute, Expr, Fields, Ident, Path, Result, Type, WherePredicate};

use super::{
    parse_assign_expr, parse_assign_from_str, parse_assign_inflection, parse_bound, parse_concrete,
    Attr, ContainerAttr, Serde, Tagged,
};
use crate::{
    attr::{parse_assign_str, EnumAttr, Inflection, VariantAttr},
    optional::{parse_optional, Optional},
    utils::{extract_docs, parse_attrs},
};

#[derive(Default, Clone)]
pub struct StructAttr {
    crate_rename: Option<Path>,
    pub type_as: Option<Type>,
    pub type_override: Option<String>,
    pub rename_all: Option<Inflection>,
    pub rename: Option<Expr>,
    pub export_to: Option<Expr>,
    pub export: bool,
    pub tag: Option<String>,
    pub docs: Vec<Expr>,
    pub concrete: HashMap<Ident, Type>,
    pub bound: Option<Vec<WherePredicate>>,
    pub optional_fields: Optional,
}

impl StructAttr {
    pub fn from_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut result = parse_attrs::<Self>(attrs)?;

        if cfg!(feature = "serde-compat") {
            let serde_attr = crate::utils::parse_serde_attrs::<StructAttr>(attrs);
            result = result.merge(serde_attr.0);
        }

        result.docs = extract_docs(attrs);

        Ok(result)
    }

    pub fn from_variant(
        enum_attr: &EnumAttr,
        variant_attr: &VariantAttr,
        variant_fields: &Fields,
    ) -> Self {
        Self {
            crate_rename: Some(enum_attr.crate_rename()),
            rename: variant_attr.rename.clone(),
            rename_all: variant_attr.rename_all.or(match variant_fields {
                Fields::Named(_) => enum_attr.rename_all_fields,
                Fields::Unnamed(_) | Fields::Unit => None,
            }),
            tag: match variant_fields {
                Fields::Named(_) => match enum_attr
                    .tagged()
                    .expect("The variant attribute is known to be valid at this point")
                {
                    Tagged::Internally { tag } => Some(tag.to_owned()),
                    _ => None,
                },
                _ => None,
            },

            // inline and skip are not supported on StructAttr
            ..Self::default()
        }
    }
}

impl Attr for StructAttr {
    type Item = Fields;

    fn merge(self, other: Self) -> Self {
        Self {
            crate_rename: self.crate_rename.or(other.crate_rename),
            type_as: self.type_as.or(other.type_as),
            type_override: self.type_override.or(other.type_override),
            rename: self.rename.or(other.rename),
            rename_all: self.rename_all.or(other.rename_all),
            export_to: self.export_to.or(other.export_to),
            export: self.export || other.export,
            tag: self.tag.or(other.tag),
            docs: other.docs,
            concrete: self.concrete.into_iter().chain(other.concrete).collect(),
            bound: match (self.bound, other.bound) {
                (Some(a), Some(b)) => Some(a.into_iter().chain(b).collect()),
                (Some(bound), None) | (None, Some(bound)) => Some(bound),
                (None, None) => None,
            },
            optional_fields: self.optional_fields.or(other.optional_fields),
        }
    }

    fn assert_validity(&self, item: &Self::Item) -> Result<()> {
        if self.type_override.is_some() {
            if self.type_as.is_some() {
                syn_err!("`as` is not compatible with `type`");
            }

            if self.rename_all.is_some() {
                syn_err!("`rename_all` is not compatible with `type`");
            }

            if self.tag.is_some() {
                syn_err!("`tag` is not compatible with `type`");
            }

            if let Optional::Optional { .. } = self.optional_fields {
                syn_err!("`optional_fields` is not compatible with `type`");
            }
        }

        if self.type_as.is_some() {
            if self.tag.is_some() {
                syn_err!("`tag` is not compatible with `as`");
            }

            if self.rename_all.is_some() {
                syn_err!("`rename_all` is not compatible with `as`");
            }

            if let Optional::Optional { .. } = self.optional_fields {
                syn_err!("`optional_fields` is not compatible with `as`");
            }
        }

        if !matches!(item, Fields::Named(_)) {
            if self.tag.is_some() {
                syn_err!("`tag` cannot be used with unit or tuple structs");
            }

            if self.rename_all.is_some() {
                syn_err!("`rename_all` cannot be used with unit or tuple structs");
            }
        }

        Ok(())
    }
}

impl ContainerAttr for StructAttr {
    fn crate_rename(&self) -> Path {
        self.crate_rename
            .clone()
            .unwrap_or_else(|| parse_quote!(::ts_rs))
    }
}

impl_parse! {
    StructAttr(input, out) {
        "crate" => out.crate_rename = Some(parse_assign_from_str(input)?),
        "as" => out.type_as = Some(parse_assign_from_str(input)?),
        "type" => out.type_override = Some(parse_assign_str(input)?),
        "rename" => out.rename = Some(parse_assign_expr(input)?),
        "rename_all" => out.rename_all = Some(parse_assign_inflection(input)?),
        "tag" => out.tag = Some(parse_assign_str(input)?),
        "export" => out.export = true,
        "export_to" => out.export_to = Some(parse_assign_expr(input)?),
        "concrete" => out.concrete = parse_concrete(input)?,
        "bound" => out.bound = Some(parse_bound(input)?),
        "optional_fields" => out.optional_fields = parse_optional(input)?,
    }
}

impl_parse! {
    Serde<StructAttr>(input, out) {
        "rename" => out.0.rename = Some(parse_assign_expr(input)?),
        "rename_all" => out.0.rename_all = Some(parse_assign_inflection(input)?),
        "tag" => out.0.tag = Some(parse_assign_str(input)?),
        "bound" => out.0.bound = Some(parse_bound(input)?),
        // parse #[serde(default)] to not emit a warning
        "deny_unknown_fields" | "default" => {
            use syn::Token;
            if input.peek(Token![=]) {
                input.parse::<Token![=]>()?;
                parse_assign_str(input)?;
            }
        },
    }
}
