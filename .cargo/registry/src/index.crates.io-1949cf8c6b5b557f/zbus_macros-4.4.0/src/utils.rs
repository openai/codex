use std::fmt::Display;

use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{Attribute, FnArg, Ident, Pat, PatIdent, PatType};

pub fn zbus_path() -> TokenStream {
    if let Ok(FoundCrate::Name(name)) = crate_name("zbus") {
        let ident = format_ident!("{}", name);
        quote! { ::#ident }
    } else {
        quote! { ::zbus }
    }
}

pub fn typed_arg(arg: &FnArg) -> Option<&PatType> {
    match arg {
        FnArg::Typed(t) => Some(t),
        _ => None,
    }
}

pub fn pat_ident(pat: &PatType) -> Option<&Ident> {
    match &*pat.pat {
        Pat::Ident(PatIdent { ident, .. }) => Some(ident),
        _ => None,
    }
}

pub fn get_doc_attrs(attrs: &[Attribute]) -> Vec<&Attribute> {
    attrs.iter().filter(|x| x.path().is_ident("doc")).collect()
}

// Convert to pascal case, assuming snake case.
// If `s` is already in pascal case, should yield the same result.
pub fn pascal_case(s: &str) -> String {
    let mut pascal = String::new();
    let mut capitalize = true;
    for ch in s.chars() {
        if ch == '_' {
            capitalize = true;
        } else if capitalize {
            pascal.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else {
            pascal.push(ch);
        }
    }
    pascal
}

pub fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

/// Standard annotation `org.freedesktop.DBus.Property.EmitsChangedSignal`.
///
/// See <https://dbus.freedesktop.org/doc/dbus-specification.html#introspection-format>.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum PropertyEmitsChangedSignal {
    #[default]
    True,
    Invalidates,
    Const,
    False,
}

impl Display for PropertyEmitsChangedSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let emits_changed_signal = match self {
            PropertyEmitsChangedSignal::True => "true",
            PropertyEmitsChangedSignal::Const => "const",
            PropertyEmitsChangedSignal::False => "false",
            PropertyEmitsChangedSignal::Invalidates => "invalidates",
        };
        write!(f, "{}", emits_changed_signal)
    }
}

impl PropertyEmitsChangedSignal {
    pub fn parse(s: &str, span: Span) -> syn::Result<Self> {
        use PropertyEmitsChangedSignal::*;

        match s {
            "true" => Ok(True),
            "invalidates" => Ok(Invalidates),
            "const" => Ok(Const),
            "false" => Ok(False),
            other => Err(syn::Error::new(
                span,
                format!("invalid value \"{other}\" for attribute `property(emits_changed_signal)`"),
            )),
        }
    }
}
