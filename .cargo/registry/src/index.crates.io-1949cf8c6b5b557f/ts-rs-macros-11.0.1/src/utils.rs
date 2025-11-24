use std::collections::HashMap;

use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{
    Attribute, Error, Expr, ExprLit, GenericParam, Generics, Lit, LitStr, Path, Result, Type,
};

use super::attr::{Attr, Serde};
use crate::deps::Dependencies;

macro_rules! syn_err {
    ($l:literal $(, $a:expr)*) => {
        syn_err!(proc_macro2::Span::call_site(); $l $(, $a)*)
    };
    ($s:expr; $l:literal $(, $a:expr)*) => {
        return Err(syn::Error::new($s, format!($l $(, $a)*)))
    };
}

macro_rules! syn_err_spanned {
    ($s:expr; $l:literal $(, $a:expr)*) => {
        return Err(syn::Error::new_spanned($s, format!($l $(, $a)*)))
    };
}

macro_rules! impl_parse {
    ($i:ident ($input:ident, $out:ident) { $($k:pat => $e:expr),* $(,)? }) => {
        impl std::convert::TryFrom<&syn::Attribute> for $i {
            type Error = syn::Error;

            fn try_from(attr: &syn::Attribute) -> syn::Result<Self> { attr.parse_args() }
        }

        impl syn::parse::Parse for $i {
            fn parse($input: syn::parse::ParseStream) -> syn::Result<Self> {
                let mut $out = Self::default();
                loop {
                    let span = $input.span();
                    let key: Ident = $input.call(syn::ext::IdentExt::parse_any)?;
                    match &*key.to_string() {
                        $($k => $e,)*
                        #[allow(unreachable_patterns)]
                        x => syn_err!(
                            span;
                            "Unknown attribute \"{x}\". Allowed attributes are: {}",
                            [$(stringify!($k),)*].join(", ")
                        )

                    }

                    if $input.is_empty() {
                        break;
                    }

                    $input.parse::<syn::Token![,]>()?;
                }

                Ok($out)
            }
        }
    };
    ($i:ident<$inner: ident> ($input:ident, $out:ident) { $($k:pat => $e:expr),* $(,)? }) => {
        impl std::convert::TryFrom<&syn::Attribute> for $i<$inner> {
            type Error = syn::Error;

            fn try_from(attr: &syn::Attribute) -> syn::Result<Self> { attr.parse_args() }
        }

        impl syn::parse::Parse for $i<$inner> {
            fn parse($input: syn::parse::ParseStream) -> syn::Result<Self> {
                let mut $out = Self::default();
                loop {
                    let key: syn::Ident = $input.call(syn::ext::IdentExt::parse_any)?;
                    match &*key.to_string() {
                        $($k => $e,)*
                        #[allow(unreachable_patterns)]
                        x => {
                            if cfg!(not(feature = "no-serde-warnings")) {
                                let tokens = crate::attr::skip_until_next_comma($input);

                                crate::utils::warning::print_warning(
                                    "failed to parse serde attribute",
                                    format!("{x} {tokens}"),
                                    "ts-rs failed to parse this attribute. It will be ignored.",
                                )
                                .unwrap();
                            } else {
                                crate::attr::skip_until_next_comma($input);
                            }
                        }
                    }

                    if $input.is_empty() {
                        break;
                    }

                    $input.parse::<syn::Token![,]>()?;
                }

                Ok($out)
            }
        }
    };
}

/// Converts a rust identifier to a typescript identifier.
pub fn to_ts_ident(ident: &Ident) -> String {
    let ident = ident.to_string();
    if ident.starts_with("r#") {
        ident.trim_start_matches("r#").to_owned()
    } else {
        ident
    }
}

/// Convert an arbitrary name to a valid Typescript field name.
///
/// If the name contains special characters or if its first character
/// is a number it will be wrapped in quotes.
pub fn raw_name_to_ts_field(value: String) -> String {
    let valid_chars = value
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '$');

    let does_not_start_with_digit = value
        .chars()
        .next()
        .map_or(true, |first| !first.is_numeric());

    let valid = valid_chars && does_not_start_with_digit;

    if valid {
        value
    } else {
        format!(r#""{value}""#)
    }
}

/// Parse all `#[ts(..)]` attributes from the given slice.
pub(crate) fn parse_attrs<'a, A>(attrs: &'a [Attribute]) -> Result<A>
where
    A: TryFrom<&'a Attribute, Error = Error> + Attr,
{
    Ok(attrs
        .iter()
        .filter(|a| a.path().is_ident("ts"))
        .map(A::try_from)
        .collect::<Result<Vec<A>>>()?
        .into_iter()
        .fold(A::default(), |acc, cur| acc.merge(cur)))
}

/// Parse all `#[serde(..)]` attributes from the given slice.
pub fn parse_serde_attrs<'a, A>(attrs: &'a [Attribute]) -> Serde<A>
where
    A: Attr,
    Serde<A>: TryFrom<&'a Attribute, Error = Error>,
{
    attrs
        .iter()
        .filter(|a| a.path().is_ident("serde"))
        .flat_map(|attr| Serde::<A>::try_from(attr).ok())
        .fold(Serde::<A>::default(), |acc, cur| acc.merge(cur))
}

/// Extracts doc comments from the given attributes, returning a `syn::Expr` for every `#[doc]`
/// attribute. These `syn::Expr`s are expected to evaluate to `&str`.
pub fn extract_docs(attrs: &[Attribute]) -> Vec<Expr> {
    attrs
        .iter()
        .filter_map(|attr| attr.meta.require_name_value().ok())
        .filter(|attr| attr.path.is_ident("doc"))
        .map(|attr| attr.value.clone())
        .collect()
}

#[cfg(feature = "serde-compat")]
pub(crate) mod warning {
    use std::{fmt::Display, io::Write};

    use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

    // Sadly, it is impossible to raise a warning in a proc macro.
    // This function prints a message which looks like a compiler warning.
    #[allow(unused)]
    pub fn print_warning(
        title: impl Display,
        content: impl Display,
        note: impl Display,
    ) -> std::io::Result<()> {
        let make_color = |color: Color, bold: bool| {
            let mut spec = ColorSpec::new();
            spec.set_fg(Some(color)).set_bold(bold).set_intense(true);
            spec
        };

        let yellow_bold = make_color(Color::Yellow, true);
        let white_bold = make_color(Color::White, true);
        let white = make_color(Color::White, false);
        let blue = make_color(Color::Blue, true);

        let writer = BufferWriter::stderr(ColorChoice::Auto);
        let mut buffer = writer.buffer();

        buffer.set_color(&yellow_bold)?;
        write!(&mut buffer, "warning")?;
        buffer.set_color(&white_bold)?;
        writeln!(&mut buffer, ": {}", title)?;

        buffer.set_color(&blue)?;
        writeln!(&mut buffer, "  | ")?;

        write!(&mut buffer, "  | ")?;
        buffer.set_color(&white)?;
        writeln!(&mut buffer, "{}", content)?;

        buffer.set_color(&blue)?;
        writeln!(&mut buffer, "  | ")?;

        write!(&mut buffer, "  = ")?;
        buffer.set_color(&white_bold)?;
        write!(&mut buffer, "note: ")?;
        buffer.set_color(&white)?;
        writeln!(&mut buffer, "{}", note)?;

        writer.print(&buffer)
    }
}
#[cfg(not(feature = "serde-compat"))]
pub(crate) mod warning {
    use std::fmt::Display;

    // Just a stub!
    #[allow(unused)]
    pub fn print_warning(
        title: impl Display,
        content: impl Display,
        note: impl Display,
    ) -> std::io::Result<()> {
        Ok(())
    }
}

/// formats the generic arguments (like A, B in struct X<A, B>{..}) as "<X>" where x is a comma
/// seperated list of generic arguments, or an empty string if there are no type generics (lifetime/const generics are ignored).
/// this expands to an expression which evaluates to a `String`.
///
/// If a default type arg is encountered, it will be added to the dependencies.
pub fn format_generics(
    deps: &mut Dependencies,
    crate_rename: &Path,
    generics: &Generics,
    concrete: &HashMap<Ident, Type>,
) -> TokenStream {
    let mut expanded_params = generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(type_param) => {
                if concrete.contains_key(&type_param.ident) {
                    return None;
                }
                let ty = type_param.ident.to_string();
                if let Some(default) = &type_param.default {
                    deps.push(default);
                    Some(quote!(
                        format!("{} = {}", #ty, <#default as #crate_rename::TS>::name())
                    ))
                } else {
                    Some(quote!(#ty.to_owned()))
                }
            }
            _ => None,
        })
        .peekable();

    if expanded_params.peek().is_none() {
        return quote!("");
    }

    let comma_separated = quote!([#(#expanded_params),*].join(", "));
    quote!(format!("<{}>", #comma_separated))
}

pub fn make_string_literal(content: &str, span: Span) -> Expr {
    Expr::Lit(ExprLit {
        attrs: vec![],
        lit: Lit::Str(LitStr::new(content, span)),
    })
}
