mod custom_meta;
mod doc;
mod parse_meta;
mod schemars_to_serde;
mod validation;

use parse_meta::{
    parse_extensions, parse_name_value_expr, parse_name_value_lit_str, require_name_value_lit_str,
    require_path_only,
};
use proc_macro2::TokenStream;
use quote::ToTokens;
use serde_derive_internals::Ctxt;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, ExprLit, Ident, Lit, LitStr, Path, Type};
use validation::ValidationAttrs;

use crate::ast::Data;
use crate::idents::SCHEMA;

pub use custom_meta::*;
pub use schemars_to_serde::process_serde_attrs;

#[derive(Default)]
pub struct CommonAttrs {
    pub doc: Option<Expr>,
    pub deprecated: bool,
    pub title: Option<Expr>,
    pub description: Option<Expr>,
    pub examples: Vec<Expr>,
    pub extensions: Vec<(String, TokenStream)>,
    pub transforms: Vec<Expr>,
}

#[derive(Default)]
pub struct FieldAttrs {
    pub common: CommonAttrs,
    pub with: Option<WithAttr>,
    pub validation: ValidationAttrs,
}

#[derive(Default)]
pub struct ContainerAttrs {
    pub common: CommonAttrs,
    pub repr: Option<Type>,
    pub crate_name: Option<Path>,
    // The actual parsing of this is done in `get_rename_format_type_params()`,
    // because it depends on the type's generic params.
    pub rename_format_string: Option<LitStr>,
    pub inline: bool,
    pub ref_variants: bool,
    pub with: Option<WithAttr>,
}

#[derive(Default)]
pub struct VariantAttrs {
    pub common: CommonAttrs,
    pub with: Option<WithAttr>,
}

pub enum WithAttr {
    Type(Type),
    Function(Path),
}

impl CommonAttrs {
    fn populate(
        &mut self,
        attrs: &[Attribute],
        schemars_cx: &mut AttrCtxt,
        serde_cx: &mut AttrCtxt,
    ) {
        self.process_attr(schemars_cx);
        self.process_attr(serde_cx);

        self.doc = doc::get_doc(attrs);
        self.deprecated = attrs.iter().any(|a| a.path().is_ident("deprecated"));
    }

    fn process_attr(&mut self, cx: &mut AttrCtxt) {
        cx.parse_meta(|m, n, c| self.process_meta(m, n, c));
    }

    fn process_meta(
        &mut self,
        meta: CustomMeta,
        meta_name: &str,
        cx: &AttrCtxt,
    ) -> Result<(), CustomMeta> {
        match meta_name {
            "title" => match self.title {
                Some(_) => cx.duplicate_error(&meta),
                None => self.title = parse_name_value_expr(meta, cx).ok(),
            },

            "description" => match self.description {
                Some(_) => cx.duplicate_error(&meta),
                None => self.description = parse_name_value_expr(meta, cx).ok(),
            },

            "example" => {
                if let Ok(expr) = parse_name_value_expr(meta, cx) {
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(lit_str),
                        ..
                    }) = &expr
                    {
                        if lit_str.parse::<Path>().is_ok() {
                            let lit_str_value = lit_str.value();
                            cx.error_spanned_by(&expr, format_args!(
                                "`example` value must be an expression, and string literals that may be interpreted as function paths are currently disallowed to avoid migration errors \
                                 (this restriction may be relaxed in a future version of schemars).\n\
                                If you want to use the result of a function, use `#[schemars(example = {lit_str_value}())]`.\n\
                                Or to use the string literal value, use `#[schemars(example = &\"{lit_str_value}\")]`."));
                        }
                    }

                    self.examples.push(expr);
                }
            }

            "extend" => {
                for ex in parse_extensions(&meta, cx).into_iter().flatten() {
                    // This is O(n^2) but should be fine with the typically small number of
                    // extensions. If this does become a problem, it can be changed to use
                    // IndexMap, or a separate Map with cloned keys.
                    if self.extensions.iter().any(|e| e.0 == ex.key_str) {
                        cx.error_spanned_by(
                            ex.key_lit,
                            format_args!("Duplicate extension key '{}'", ex.key_str),
                        );
                    } else {
                        self.extensions.push((ex.key_str, ex.value));
                    }
                }
            }

            "transform" => {
                if let Ok(expr) = parse_name_value_expr(meta, cx) {
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(lit_str),
                        ..
                    }) = &expr
                    {
                        if lit_str.parse::<Expr>().is_ok() {
                            cx.error_spanned_by(
                                &expr,
                                format_args!(
                                    "Expected a `fn(&mut Schema)` or other value implementing `schemars::transform::Transform`, found `&str`.\nDid you mean `#[schemars(transform = {})]`?",
                                    lit_str.value()
                                ),
                            );
                        }
                    } else {
                        self.transforms.push(expr);
                    }
                }
            }

            _ => return Err(meta),
        }

        Ok(())
    }

    pub fn is_default(&self) -> bool {
        matches!(
            self,
            Self {
                title: None,
                description: None,
                doc: None,
                deprecated: false,
                examples,
                extensions,
                transforms,
            } if examples.is_empty() && extensions.is_empty() && transforms.is_empty()
        )
    }

    pub fn add_mutators(&self, mutators: &mut Vec<TokenStream>) {
        let mut title = self.title.as_ref().map(ToTokens::to_token_stream);
        let mut description = self.description.as_ref().map(ToTokens::to_token_stream);
        if let Some(doc) = &self.doc {
            title.get_or_insert_with(|| {
                quote!({
                    const TITLE: &str = schemars::_private::get_title_and_description(#doc).0;
                    TITLE
                })
            });
            description.get_or_insert_with(|| {
                quote!({
                    const DESCRIPTION: &str = schemars::_private::get_title_and_description(#doc).1;
                    DESCRIPTION
                })
            });
        }
        if let Some(title) = title {
            mutators.push(quote! {
                schemars::_private::insert_metadata_property_if_nonempty(&mut #SCHEMA, "title", #title);
            });
        }
        if let Some(description) = description {
            mutators.push(quote! {
                schemars::_private::insert_metadata_property_if_nonempty(&mut #SCHEMA, "description", #description);
            });
        }

        if self.deprecated {
            mutators.push(quote! {
                #SCHEMA.insert("deprecated".into(), true.into());
            });
        }

        if !self.examples.is_empty() {
            let examples = self.examples.iter().map(|eg| {
                quote! {
                    schemars::_private::serde_json::value::to_value(#eg)
                }
            });
            mutators.push(quote! {
                #SCHEMA.insert("examples".into(), schemars::_private::serde_json::Value::Array([#(#examples),*].into_iter().flatten().collect()));
            });
        }

        for (k, v) in &self.extensions {
            mutators.push(quote! {
                #SCHEMA.insert(#k.into(), schemars::_private::serde_json::json!(#v));
            });
        }

        for transform in &self.transforms {
            mutators.push(quote_spanned! {transform.span()=>
                schemars::transform::Transform::transform(&mut #transform, &mut #SCHEMA);
            });
        }
    }
}

impl FieldAttrs {
    pub fn new(attrs: &[Attribute], cx: &Ctxt) -> Self {
        let mut result = Self::default();
        result.populate(attrs, cx);
        result
    }

    fn populate(&mut self, attrs: &[Attribute], cx: &Ctxt) {
        let schemars_cx = &mut AttrCtxt::new(cx, attrs, "schemars");
        let serde_cx = &mut AttrCtxt::new(cx, attrs, "serde");
        let validate_cx = &mut AttrCtxt::new(cx, attrs, "validate");
        let garde_cx = &mut AttrCtxt::new(cx, attrs, "garde");

        self.common.populate(attrs, schemars_cx, serde_cx);
        self.validation.populate(schemars_cx, validate_cx, garde_cx);
        self.process_attr(schemars_cx);
        self.process_attr(serde_cx);
    }

    fn process_attr(&mut self, cx: &mut AttrCtxt) {
        cx.parse_meta(|m, n, c| self.process_meta(m, n, c));
    }

    fn process_meta(
        &mut self,
        meta: CustomMeta,
        meta_name: &str,
        cx: &AttrCtxt,
    ) -> Result<(), CustomMeta> {
        match meta_name {
            "with" => match self.with {
                Some(WithAttr::Type(_)) => cx.duplicate_error(&meta),
                Some(WithAttr::Function(_)) => cx.mutual_exclusive_error(&meta, "schema_with"),
                None => self.with = parse_name_value_lit_str(meta, cx).ok().map(WithAttr::Type),
            },
            "schema_with" if cx.attr_type == "schemars" => match self.with {
                Some(WithAttr::Function(_)) => cx.duplicate_error(&meta),
                Some(WithAttr::Type(_)) => cx.mutual_exclusive_error(&meta, "with"),
                None => {
                    self.with = parse_name_value_lit_str(meta, cx)
                        .ok()
                        .map(WithAttr::Function);
                }
            },

            _ => return Err(meta),
        }

        Ok(())
    }

    pub fn is_default(&self) -> bool {
        matches!(
            self,
            Self {
                common,
                validation,
                with: None,
            } if common.is_default() && validation.is_default())
    }
}

impl ContainerAttrs {
    pub fn new(attrs: &[Attribute], data: &Data, cx: &Ctxt) -> Self {
        let mut result = Self::default();
        result.populate(attrs, data, cx);
        result
    }

    fn populate(&mut self, attrs: &[Attribute], data: &Data, cx: &Ctxt) {
        let schemars_cx = &mut AttrCtxt::new(cx, attrs, "schemars");
        let serde_cx = &mut AttrCtxt::new(cx, attrs, "serde");

        self.common.populate(attrs, schemars_cx, serde_cx);
        self.process_attr(data, schemars_cx);
        self.process_attr(data, serde_cx);

        self.repr = attrs
            .iter()
            .find(|a| a.path().is_ident("repr"))
            .and_then(|a| a.parse_args().ok());
    }

    fn process_attr(&mut self, data: &Data, cx: &mut AttrCtxt) {
        cx.parse_meta(|m, n, c| self.process_meta(m, n, data, c));
    }

    fn process_meta(
        &mut self,
        meta: CustomMeta,
        meta_name: &str,
        data: &Data,
        cx: &AttrCtxt,
    ) -> Result<(), CustomMeta> {
        match meta_name {
            "crate" => match self.crate_name {
                Some(_) => cx.duplicate_error(&meta),
                None => self.crate_name = parse_name_value_lit_str(meta, cx).ok(),
            },

            "rename" if cx.attr_type == "schemars" => match self.rename_format_string {
                Some(_) => cx.duplicate_error(&meta),
                None => self.rename_format_string = require_name_value_lit_str(meta, cx).ok(),
            },

            "inline" => {
                if self.inline {
                    cx.duplicate_error(&meta);
                } else if require_path_only(&meta, cx).is_ok() {
                    self.inline = true;
                }
            }

            "with" if cx.attr_type == "schemars" => match self.with {
                Some(WithAttr::Type(_)) => cx.duplicate_error(&meta),
                Some(WithAttr::Function(_)) => cx.mutual_exclusive_error(&meta, "schema_with"),
                None => self.with = parse_name_value_lit_str(meta, cx).ok().map(WithAttr::Type),
            },
            "schema_with" if cx.attr_type == "schemars" => match self.with {
                Some(WithAttr::Function(_)) => cx.duplicate_error(&meta),
                Some(WithAttr::Type(_)) => cx.mutual_exclusive_error(&meta, "with"),
                None => {
                    self.with = parse_name_value_lit_str(meta, cx)
                        .ok()
                        .map(WithAttr::Function);
                }
            },

            "_unstable_ref_variants" if cx.attr_type == "schemars" => {
                if !matches!(data, Data::Enum(_)) {
                    cx.error_spanned_by(
                        meta.path(),
                        "`_unstable_ref_variants` can only be used on enums",
                    );
                } else if self.ref_variants {
                    cx.duplicate_error(&meta);
                } else if require_path_only(&meta, cx).is_ok() {
                    self.ref_variants = true;
                }
            }

            _ => return Err(meta),
        }

        Ok(())
    }
}

impl VariantAttrs {
    pub fn new(attrs: &[Attribute], cx: &Ctxt) -> Self {
        let mut result = Self::default();
        result.populate(attrs, cx);
        result
    }

    fn populate(&mut self, attrs: &[Attribute], cx: &Ctxt) {
        let schemars_cx = &mut AttrCtxt::new(cx, attrs, "schemars");
        let serde_cx = &mut AttrCtxt::new(cx, attrs, "serde");

        self.common.populate(attrs, schemars_cx, serde_cx);
        self.process_attr(schemars_cx);
        self.process_attr(serde_cx);
    }

    fn process_attr(&mut self, cx: &mut AttrCtxt) {
        cx.parse_meta(|m, n, c| self.process_meta(m, n, c));
    }

    fn process_meta(
        &mut self,
        meta: CustomMeta,
        meta_name: &str,
        cx: &AttrCtxt,
    ) -> Result<(), CustomMeta> {
        match meta_name {
            "with" => match self.with {
                Some(WithAttr::Type(_)) => cx.duplicate_error(&meta),
                Some(WithAttr::Function(_)) => cx.mutual_exclusive_error(&meta, "schema_with"),
                None => self.with = parse_name_value_lit_str(meta, cx).ok().map(WithAttr::Type),
            },
            "schema_with" if cx.attr_type == "schemars" => match self.with {
                Some(WithAttr::Function(_)) => cx.duplicate_error(&meta),
                Some(WithAttr::Type(_)) => cx.mutual_exclusive_error(&meta, "with"),
                None => {
                    self.with = parse_name_value_lit_str(meta, cx)
                        .ok()
                        .map(WithAttr::Function);
                }
            },

            _ => return Err(meta),
        }

        Ok(())
    }

    pub fn is_default(&self) -> bool {
        matches!(
            self,
            Self {
                common,
                with: None,
            } if common.is_default()
        )
    }
}

fn get_meta_items(attrs: &[Attribute], attr_type: &'static str, cx: &Ctxt) -> Vec<CustomMeta> {
    let mut result = vec![];

    for attr in attrs.iter().filter(|a| a.path().is_ident(attr_type)) {
        match attr.parse_args_with(Punctuated::<CustomMeta, Token![,]>::parse_terminated) {
            Ok(list) => result.extend(list),
            Err(err) => {
                if attr_type == "schemars" {
                    cx.syn_error(err);
                }
            }
        }
    }

    result
}

fn path_str(path: &Path) -> String {
    path.get_ident().map_or_else(
        || path.into_token_stream().to_string().replace(' ', ""),
        Ident::to_string,
    )
}

pub struct AttrCtxt<'a> {
    inner: &'a Ctxt,
    attr_type: &'static str,
    metas: Vec<CustomMeta>,
}

impl<'a> AttrCtxt<'a> {
    pub fn new(inner: &'a Ctxt, attrs: &'a [Attribute], attr_type: &'static str) -> Self {
        Self {
            inner,
            attr_type,
            metas: get_meta_items(attrs, attr_type, inner),
        }
    }

    pub fn new_nested_meta(&self, metas: Vec<CustomMeta>) -> Self {
        Self { metas, ..*self }
    }

    pub fn parse_meta(
        &mut self,
        mut handle: impl FnMut(CustomMeta, &str, &Self) -> Result<(), CustomMeta>,
    ) {
        let metas = std::mem::take(&mut self.metas);
        self.metas = metas
            .into_iter()
            .filter_map(|meta| match meta.path().get_ident().map(Ident::to_string) {
                Some(ident) => handle(meta, &ident, self).err(),
                _ => Some(meta),
            })
            .collect();
    }

    pub fn error_spanned_by<A: ToTokens, T: std::fmt::Display>(&self, obj: A, msg: T) {
        self.inner.error_spanned_by(obj, msg);
    }

    pub fn syn_error(&self, err: syn::Error) {
        self.inner.syn_error(err);
    }

    pub fn mutual_exclusive_error(&self, meta: &CustomMeta, other_attr: &str) {
        if self.attr_type == "schemars" {
            self.error_spanned_by(
                meta,
                format_args!(
                    "schemars attribute cannot contain both `{}` and `{}`",
                    path_str(meta.path()),
                    other_attr,
                ),
            );
        }
    }

    pub fn duplicate_error(&self, meta: &CustomMeta) {
        if self.attr_type == "schemars" {
            self.error_spanned_by(
                meta,
                format_args!(
                    "duplicate schemars attribute item `{}`",
                    path_str(meta.path())
                ),
            );
        }
    }
}

impl Drop for AttrCtxt<'_> {
    fn drop(&mut self) {
        if self.attr_type == "schemars" {
            for unhandled_meta in self.metas.iter().filter(|m| !is_schemars_serde_keyword(m)) {
                self.error_spanned_by(
                    unhandled_meta.path(),
                    format_args!(
                        "unknown schemars attribute `{}`",
                        path_str(unhandled_meta.path())
                    ),
                );
            }
        }
    }
}

fn is_schemars_serde_keyword(meta: &CustomMeta) -> bool {
    let known_keywords = schemars_to_serde::SCHEMARS_KEYWORDS_PARSED_BY_SERDE;
    meta.path()
        .get_ident()
        .is_some_and(|i| known_keywords.contains(&i.to_string().as_str()))
}
