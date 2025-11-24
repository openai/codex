use std::iter::FromIterator;

use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Attribute, Expr, Ident, LitStr, Token,
};

use crate::utils::Sp;

#[derive(Clone)]
pub(crate) struct ClapAttr {
    pub(crate) kind: Sp<AttrKind>,
    pub(crate) name: Ident,
    pub(crate) magic: Option<MagicAttrName>,
    pub(crate) value: Option<AttrValue>,
}

impl ClapAttr {
    pub(crate) fn parse_all(all_attrs: &[Attribute]) -> Result<Vec<Self>, syn::Error> {
        let mut parsed = Vec::new();
        for attr in all_attrs {
            let kind = if attr.path().is_ident("clap") {
                Sp::new(AttrKind::Clap, attr.path().span())
            } else if attr.path().is_ident("structopt") {
                Sp::new(AttrKind::StructOpt, attr.path().span())
            } else if attr.path().is_ident("command") {
                Sp::new(AttrKind::Command, attr.path().span())
            } else if attr.path().is_ident("group") {
                Sp::new(AttrKind::Group, attr.path().span())
            } else if attr.path().is_ident("arg") {
                Sp::new(AttrKind::Arg, attr.path().span())
            } else if attr.path().is_ident("value") {
                Sp::new(AttrKind::Value, attr.path().span())
            } else {
                continue;
            };
            for mut attr in
                attr.parse_args_with(Punctuated::<ClapAttr, Token![,]>::parse_terminated)?
            {
                attr.kind = kind;
                parsed.push(attr);
            }
        }
        Ok(parsed)
    }

    pub(crate) fn value_or_abort(&self) -> Result<&AttrValue, syn::Error> {
        self.value
            .as_ref()
            .ok_or_else(|| format_err!(self.name, "attribute `{}` requires a value", self.name))
    }

    pub(crate) fn lit_str_or_abort(&self) -> Result<&LitStr, syn::Error> {
        let value = self.value_or_abort()?;
        match value {
            AttrValue::LitStr(tokens) => Ok(tokens),
            AttrValue::Expr(_) | AttrValue::Call(_) => {
                abort!(
                    self.name,
                    "attribute `{}` can only accept string literals",
                    self.name
                )
            }
        }
    }
}

impl Parse for ClapAttr {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        let magic = match name_str.as_str() {
            "rename_all" => Some(MagicAttrName::RenameAll),
            "rename_all_env" => Some(MagicAttrName::RenameAllEnv),
            "skip" => Some(MagicAttrName::Skip),
            "next_display_order" => Some(MagicAttrName::NextDisplayOrder),
            "next_help_heading" => Some(MagicAttrName::NextHelpHeading),
            "default_value_t" => Some(MagicAttrName::DefaultValueT),
            "default_values_t" => Some(MagicAttrName::DefaultValuesT),
            "default_value_os_t" => Some(MagicAttrName::DefaultValueOsT),
            "default_values_os_t" => Some(MagicAttrName::DefaultValuesOsT),
            "long" => Some(MagicAttrName::Long),
            "short" => Some(MagicAttrName::Short),
            "value_parser" => Some(MagicAttrName::ValueParser),
            "action" => Some(MagicAttrName::Action),
            "env" => Some(MagicAttrName::Env),
            "flatten" => Some(MagicAttrName::Flatten),
            "value_enum" => Some(MagicAttrName::ValueEnum),
            "from_global" => Some(MagicAttrName::FromGlobal),
            "subcommand" => Some(MagicAttrName::Subcommand),
            "external_subcommand" => Some(MagicAttrName::ExternalSubcommand),
            "verbatim_doc_comment" => Some(MagicAttrName::VerbatimDocComment),
            "about" => Some(MagicAttrName::About),
            "long_about" => Some(MagicAttrName::LongAbout),
            "long_help" => Some(MagicAttrName::LongHelp),
            "author" => Some(MagicAttrName::Author),
            "version" => Some(MagicAttrName::Version),
            _ => None,
        };

        let value = if input.peek(Token![=]) {
            // `name = value` attributes.
            let assign_token = input.parse::<Token![=]>()?; // skip '='
            if input.peek(LitStr) {
                let lit: LitStr = input.parse()?;
                Some(AttrValue::LitStr(lit))
            } else {
                match input.parse::<Expr>() {
                    Ok(expr) => Some(AttrValue::Expr(expr)),

                    Err(_) => abort! {
                        assign_token,
                        "expected `string literal` or `expression` after `=`"
                    },
                }
            }
        } else if input.peek(syn::token::Paren) {
            // `name(...)` attributes.
            let nested;
            parenthesized!(nested in input);

            let method_args: Punctuated<_, _> = nested.parse_terminated(Expr::parse, Token![,])?;
            Some(AttrValue::Call(Vec::from_iter(method_args)))
        } else {
            None
        };

        Ok(Self {
            kind: Sp::new(AttrKind::Clap, name.span()),
            name,
            magic,
            value,
        })
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum MagicAttrName {
    Short,
    Long,
    ValueParser,
    Action,
    Env,
    Flatten,
    ValueEnum,
    FromGlobal,
    Subcommand,
    VerbatimDocComment,
    ExternalSubcommand,
    About,
    LongAbout,
    LongHelp,
    Author,
    Version,
    RenameAllEnv,
    RenameAll,
    Skip,
    DefaultValueT,
    DefaultValuesT,
    DefaultValueOsT,
    DefaultValuesOsT,
    NextDisplayOrder,
    NextHelpHeading,
}

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum AttrValue {
    LitStr(LitStr),
    Expr(Expr),
    Call(Vec<Expr>),
}

impl ToTokens for AttrValue {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::LitStr(t) => t.to_tokens(tokens),
            Self::Expr(t) => t.to_tokens(tokens),
            Self::Call(t) => {
                let t = quote!(#(#t),*);
                t.to_tokens(tokens);
            }
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum AttrKind {
    Clap,
    StructOpt,
    Command,
    Group,
    Arg,
    Value,
}

impl AttrKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Clap => "clap",
            Self::StructOpt => "structopt",
            Self::Command => "command",
            Self::Group => "group",
            Self::Arg => "arg",
            Self::Value => "value",
        }
    }
}
