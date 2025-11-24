use std::collections::HashMap;

use syn::{parse_quote, Attribute, Expr, Ident, ItemEnum, Path, Result, Type, WherePredicate};

use super::{parse_assign_expr, parse_assign_from_str, parse_bound, Attr, ContainerAttr, Serde};
use crate::{
    attr::{parse_assign_inflection, parse_assign_str, parse_concrete, Inflection},
    utils::{extract_docs, parse_attrs},
};

#[derive(Default)]
pub struct EnumAttr {
    crate_rename: Option<Path>,
    pub type_as: Option<Type>,
    pub type_override: Option<String>,
    pub rename_all: Option<Inflection>,
    pub rename_all_fields: Option<Inflection>,
    pub rename: Option<Expr>,
    pub export_to: Option<Expr>,
    pub export: bool,
    pub docs: Vec<Expr>,
    pub concrete: HashMap<Ident, Type>,
    pub bound: Option<Vec<WherePredicate>>,
    pub tag: Option<String>,
    pub untagged: bool,
    pub content: Option<String>,
}

#[derive(Copy, Clone)]
pub enum Tagged<'a> {
    Externally,
    Adjacently { tag: &'a str, content: &'a str },
    Internally { tag: &'a str },
    Untagged,
}

impl EnumAttr {
    pub fn tagged(&self) -> Result<Tagged<'_>> {
        match (self.untagged, &self.tag, &self.content) {
            (false, None, None) => Ok(Tagged::Externally),
            (false, Some(tag), None) => Ok(Tagged::Internally { tag }),
            (false, Some(tag), Some(content)) => Ok(Tagged::Adjacently { tag, content }),
            (true, None, None) => Ok(Tagged::Untagged),
            (true, Some(_), None) => syn_err!("untagged cannot be used with tag"),
            (true, _, Some(_)) => syn_err!("untagged cannot be used with content"),
            (false, None, Some(_)) => syn_err!("content cannot be used without tag"),
        }
    }

    pub fn from_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut result = parse_attrs::<Self>(attrs)?;

        if cfg!(feature = "serde-compat") {
            let serde_attr = crate::utils::parse_serde_attrs::<EnumAttr>(attrs);
            result = result.merge(serde_attr.0);
        }

        result.docs = extract_docs(attrs);

        Ok(result)
    }

    pub fn crate_rename(&self) -> Path {
        self.crate_rename
            .clone()
            .unwrap_or_else(|| parse_quote!(::ts_rs))
    }
}

impl Attr for EnumAttr {
    type Item = ItemEnum;

    fn merge(self, other: Self) -> Self {
        Self {
            crate_rename: self.crate_rename.or(other.crate_rename),
            type_as: self.type_as.or(other.type_as),
            type_override: self.type_override.or(other.type_override),
            rename: self.rename.or(other.rename),
            rename_all: self.rename_all.or(other.rename_all),
            rename_all_fields: self.rename_all_fields.or(other.rename_all_fields),
            tag: self.tag.or(other.tag),
            untagged: self.untagged || other.untagged,
            content: self.content.or(other.content),
            export: self.export || other.export,
            export_to: self.export_to.or(other.export_to),
            docs: other.docs,
            concrete: self.concrete.into_iter().chain(other.concrete).collect(),
            bound: match (self.bound, other.bound) {
                (Some(a), Some(b)) => Some(a.into_iter().chain(b).collect()),
                (Some(bound), None) | (None, Some(bound)) => Some(bound),
                (None, None) => None,
            },
        }
    }

    fn assert_validity(&self, item: &Self::Item) -> Result<()> {
        if self.type_override.is_some() {
            if self.type_as.is_some() {
                syn_err_spanned!(
                    item;
                    "`as` is not compatible with `type`"
                );
            }

            if self.rename_all.is_some() {
                syn_err_spanned!(
                    item;
                    "`rename_all` is not compatible with `type`"
                );
            }

            if self.rename_all_fields.is_some() {
                syn_err_spanned!(
                    item;
                    "`rename_all_fields` is not compatible with `type`"
                );
            }

            if self.tag.is_some() {
                syn_err_spanned!(
                    item;
                    "`tag` is not compatible with `type`"
                );
            }

            if self.content.is_some() {
                syn_err_spanned!(
                    item;
                    "`content` is not compatible with `type`"
                );
            }

            if self.untagged {
                syn_err_spanned!(
                    item;
                    "`untagged` is not compatible with `type`"
                );
            }
        }

        if self.type_as.is_some() {
            if self.rename_all.is_some() {
                syn_err_spanned!(
                    item;
                    "`rename_all` is not compatible with `as`"
                );
            }

            if self.rename_all_fields.is_some() {
                syn_err_spanned!(
                    item;
                    "`rename_all_fields` is not compatible with `as`"
                );
            }

            if self.tag.is_some() {
                syn_err_spanned!(
                    item;
                    "`tag` is not compatible with `as`"
                );
            }

            if self.content.is_some() {
                syn_err_spanned!(
                    item;
                    "`content` is not compatible with `as`"
                );
            }

            if self.untagged {
                syn_err_spanned!(
                    item;
                    "`untagged` is not compatible with `as`"
                );
            }
        }

        match (self.untagged, &self.tag, &self.content) {
            (true, Some(_), None) => syn_err_spanned!(
                item;
                "untagged cannot be used with tag"
            ),
            (true, _, Some(_)) => syn_err_spanned!(
                item;
                "untagged cannot be used with content"
            ),
            (false, None, Some(_)) => syn_err_spanned!(
                item;
                "content cannot be used without tag"
            ),
            _ => (),
        };

        Ok(())
    }
}

impl ContainerAttr for EnumAttr {
    fn crate_rename(&self) -> Path {
        self.crate_rename
            .clone()
            .unwrap_or_else(|| parse_quote!(::ts_rs))
    }
}

impl_parse! {
    EnumAttr(input, out) {
        "crate" => out.crate_rename = Some(parse_assign_from_str(input)?),
        "as" => out.type_as = Some(parse_assign_from_str(input)?),
        "type" => out.type_override = Some(parse_assign_str(input)?),
        "rename" => out.rename = Some(parse_assign_expr(input)?),
        "rename_all" => out.rename_all = Some(parse_assign_inflection(input)?),
        "rename_all_fields" => out.rename_all_fields = Some(parse_assign_inflection(input)?),
        "export_to" => out.export_to = Some(parse_assign_expr(input)?),
        "export" => out.export = true,
        "tag" => out.tag = Some(parse_assign_str(input)?),
        "content" => out.content = Some(parse_assign_str(input)?),
        "untagged" => out.untagged = true,
        "concrete" => out.concrete = parse_concrete(input)?,
        "bound" => out.bound = Some(parse_bound(input)?),
    }
}

impl_parse! {
    Serde<EnumAttr>(input, out) {
        "rename" => out.0.rename = Some(parse_assign_expr(input)?),
        "rename_all" => out.0.rename_all = Some(parse_assign_inflection(input)?),
        "rename_all_fields" => out.0.rename_all_fields = Some(parse_assign_inflection(input)?),
        "tag" => out.0.tag = Some(parse_assign_str(input)?),
        "content" => out.0.content = Some(parse_assign_str(input)?),
        "untagged" => out.0.untagged = true,
        "bound" => out.0.bound = Some(parse_bound(input)?),
    }
}
