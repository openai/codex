mod doc;
mod schemars_to_serde;
mod validation;

pub use schemars_to_serde::process_serde_attrs;
pub use validation::ValidationAttrs;

use crate::metadata::SchemaMetadata;
use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::ToTokens;
use serde_derive_internals::Ctxt;
use syn::parse::{self, Parse};
use syn::{Meta, MetaNameValue};

// FIXME using the same struct for containers+variants+fields means that
//  with/schema_with are accepted (but ignored) on containers, and
//  repr/crate_name are accepted (but ignored) on variants and fields etc.

#[derive(Debug, Default)]
pub struct Attrs {
    pub with: Option<WithAttr>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub deprecated: bool,
    pub examples: Vec<syn::Path>,
    pub repr: Option<syn::Type>,
    pub crate_name: Option<syn::Path>,
    pub is_renamed: bool,
}

#[derive(Debug)]
pub enum WithAttr {
    Type(syn::Type),
    Function(syn::Path),
}

impl Attrs {
    pub fn new(attrs: &[syn::Attribute], errors: &Ctxt) -> Self {
        let mut result = Attrs::default()
            .populate(attrs, "schemars", false, errors)
            .populate(attrs, "serde", true, errors);

        result.deprecated = attrs.iter().any(|a| a.path().is_ident("deprecated"));
        result.repr = attrs
            .iter()
            .find(|a| a.path().is_ident("repr"))
            .and_then(|a| a.parse_args().ok());

        let (doc_title, doc_description) = doc::get_title_and_desc_from_doc(attrs);
        result.title = result.title.or(doc_title);
        result.description = result.description.or(doc_description);

        result
    }

    pub fn as_metadata(&self) -> SchemaMetadata<'_> {
        #[allow(clippy::ptr_arg)]
        fn none_if_empty(s: &String) -> Option<&str> {
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }

        SchemaMetadata {
            title: self.title.as_ref().and_then(none_if_empty),
            description: self.description.as_ref().and_then(none_if_empty),
            deprecated: self.deprecated,
            examples: &self.examples,
            read_only: false,
            write_only: false,
            default: None,
        }
    }

    fn populate(
        mut self,
        attrs: &[syn::Attribute],
        attr_type: &'static str,
        ignore_errors: bool,
        errors: &Ctxt,
    ) -> Self {
        let duplicate_error = |meta: &MetaNameValue| {
            if !ignore_errors {
                let msg = format!(
                    "duplicate schemars attribute `{}`",
                    meta.path.get_ident().unwrap()
                );
                errors.error_spanned_by(meta, msg)
            }
        };
        let mutual_exclusive_error = |meta: &MetaNameValue, other: &str| {
            if !ignore_errors {
                let msg = format!(
                    "schemars attribute cannot contain both `{}` and `{}`",
                    meta.path.get_ident().unwrap(),
                    other,
                );
                errors.error_spanned_by(meta, msg)
            }
        };

        for meta_item in get_meta_items(attrs, attr_type, errors, ignore_errors) {
            match &meta_item {
                Meta::NameValue(m) if m.path.is_ident("with") => {
                    if let Ok(ty) = parse_lit_into_ty(errors, attr_type, "with", &m.value) {
                        match self.with {
                            Some(WithAttr::Type(_)) => duplicate_error(m),
                            Some(WithAttr::Function(_)) => mutual_exclusive_error(m, "schema_with"),
                            None => self.with = Some(WithAttr::Type(ty)),
                        }
                    }
                }

                Meta::NameValue(m) if m.path.is_ident("schema_with") => {
                    if let Ok(fun) = parse_lit_into_path(errors, attr_type, "schema_with", &m.value)
                    {
                        match self.with {
                            Some(WithAttr::Function(_)) => duplicate_error(m),
                            Some(WithAttr::Type(_)) => mutual_exclusive_error(m, "with"),
                            None => self.with = Some(WithAttr::Function(fun)),
                        }
                    }
                }

                Meta::NameValue(m) if m.path.is_ident("title") => {
                    if let Ok(title) = expr_as_lit_str(errors, attr_type, "title", &m.value) {
                        match self.title {
                            Some(_) => duplicate_error(m),
                            None => self.title = Some(title.value()),
                        }
                    }
                }

                Meta::NameValue(m) if m.path.is_ident("description") => {
                    if let Ok(description) =
                        expr_as_lit_str(errors, attr_type, "description", &m.value)
                    {
                        match self.description {
                            Some(_) => duplicate_error(m),
                            None => self.description = Some(description.value()),
                        }
                    }
                }

                Meta::NameValue(m) if m.path.is_ident("example") => {
                    if let Ok(fun) = parse_lit_into_path(errors, attr_type, "example", &m.value) {
                        self.examples.push(fun)
                    }
                }

                Meta::NameValue(m) if m.path.is_ident("rename") => self.is_renamed = true,

                Meta::NameValue(m) if m.path.is_ident("crate") && attr_type == "schemars" => {
                    if let Ok(p) = parse_lit_into_path(errors, attr_type, "crate", &m.value) {
                        if self.crate_name.is_some() {
                            duplicate_error(m)
                        } else {
                            self.crate_name = Some(p)
                        }
                    }
                }

                _ if ignore_errors => {}

                Meta::List(m) if m.path.is_ident("inner") && attr_type == "schemars" => {
                    // This will be processed with the validation attributes.
                    // It's allowed only for the schemars attribute because the
                    // validator crate doesn't support it yet.
                }

                _ => {
                    if !is_known_serde_or_validation_keyword(&meta_item) {
                        let path = meta_item
                            .path()
                            .into_token_stream()
                            .to_string()
                            .replace(' ', "");
                        errors.error_spanned_by(
                            meta_item.path(),
                            format!("unknown schemars attribute `{}`", path),
                        );
                    }
                }
            }
        }
        self
    }

    pub fn is_default(&self) -> bool {
        matches!(self, Self {
                with: None,
                title: None,
                description: None,
                deprecated: false,
                examples,
                repr: None,
                crate_name: None,
                is_renamed: _,
            } if examples.is_empty())
    }
}

fn is_known_serde_or_validation_keyword(meta: &syn::Meta) -> bool {
    let mut known_keywords = schemars_to_serde::SERDE_KEYWORDS
        .iter()
        .chain(validation::VALIDATION_KEYWORDS);
    meta.path()
        .get_ident()
        .map(|i| known_keywords.any(|k| i == k))
        .unwrap_or(false)
}

fn get_meta_items(
    attrs: &[syn::Attribute],
    attr_type: &'static str,
    errors: &Ctxt,
    ignore_errors: bool,
) -> Vec<Meta> {
    let mut result = vec![];
    for attr in attrs.iter().filter(|a| a.path().is_ident(attr_type)) {
        match attr.parse_args_with(syn::punctuated::Punctuated::<Meta, Token![,]>::parse_terminated)
        {
            Ok(list) => result.extend(list),
            Err(err) if !ignore_errors => errors.syn_error(err),
            Err(_) => {}
        }
    }

    result
}

fn expr_as_lit_str<'a>(
    cx: &Ctxt,
    attr_type: &'static str,
    meta_item_name: &'static str,
    expr: &'a syn::Expr,
) -> Result<&'a syn::LitStr, ()> {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(lit_str),
        ..
    }) = expr
    {
        Ok(lit_str)
    } else {
        cx.error_spanned_by(
            expr,
            format!(
                "expected {} {} attribute to be a string: `{} = \"...\"`",
                attr_type, meta_item_name, meta_item_name
            ),
        );
        Err(())
    }
}

fn parse_lit_into_ty(
    cx: &Ctxt,
    attr_type: &'static str,
    meta_item_name: &'static str,
    lit: &syn::Expr,
) -> Result<syn::Type, ()> {
    let string = expr_as_lit_str(cx, attr_type, meta_item_name, lit)?;

    parse_lit_str(string).map_err(|_| {
        cx.error_spanned_by(
            lit,
            format!(
                "failed to parse type: `{} = {:?}`",
                meta_item_name,
                string.value()
            ),
        )
    })
}

fn parse_lit_into_path(
    cx: &Ctxt,
    attr_type: &'static str,
    meta_item_name: &'static str,
    expr: &syn::Expr,
) -> Result<syn::Path, ()> {
    let lit_str = expr_as_lit_str(cx, attr_type, meta_item_name, expr)?;

    parse_lit_str(lit_str).map_err(|_| {
        cx.error_spanned_by(
            expr,
            format!(
                "failed to parse path: `{} = {:?}`",
                meta_item_name,
                lit_str.value()
            ),
        )
    })
}

fn parse_lit_str<T>(s: &syn::LitStr) -> parse::Result<T>
where
    T: Parse,
{
    let tokens = spanned_tokens(s)?;
    syn::parse2(tokens)
}

fn spanned_tokens(s: &syn::LitStr) -> parse::Result<TokenStream> {
    let stream = syn::parse_str(&s.value())?;
    Ok(respan_token_stream(stream, s.span()))
}

fn respan_token_stream(stream: TokenStream, span: Span) -> TokenStream {
    stream
        .into_iter()
        .map(|token| respan_token_tree(token, span))
        .collect()
}

fn respan_token_tree(mut token: TokenTree, span: Span) -> TokenTree {
    if let TokenTree::Group(g) = &mut token {
        *g = Group::new(g.delimiter(), respan_token_stream(g.stream(), span));
    }
    token.set_span(span);
    token
}
