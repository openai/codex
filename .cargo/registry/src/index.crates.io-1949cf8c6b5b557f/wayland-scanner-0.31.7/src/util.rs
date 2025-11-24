use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};

pub(crate) fn to_doc_attr(text: &str) -> TokenStream {
    let text = text.lines().map(str::trim).collect::<Vec<_>>().join("\n");
    let text = text.trim();

    quote!(#[doc = #text])
}

pub(crate) fn description_to_doc_attr((short, long): &(String, String)) -> TokenStream {
    to_doc_attr(&format!("{short}\n\n{long}"))
}

pub fn is_keyword(txt: &str) -> bool {
    matches!(
        txt,
        "abstract"
            | "alignof"
            | "as"
            | "become"
            | "box"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "do"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "final"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "macro"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "offsetof"
            | "override"
            | "priv"
            | "proc"
            | "pub"
            | "pure"
            | "ref"
            | "return"
            | "Self"
            | "self"
            | "sizeof"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "typeof"
            | "unsafe"
            | "unsized"
            | "use"
            | "virtual"
            | "where"
            | "while"
            | "yield"
            | "__handler"
            | "__object"
    )
}

pub fn is_camel_keyword(txt: &str) -> bool {
    matches!(txt, "Self")
}

pub fn snake_to_camel(input: &str) -> String {
    let result = input
        .split('_')
        .flat_map(|s| {
            let mut first = true;
            s.chars().map(move |c| {
                if first {
                    first = false;
                    c.to_ascii_uppercase()
                } else {
                    c
                }
            })
        })
        .collect::<String>();

    if is_camel_keyword(&result) {
        format!("_{}", &result)
    } else {
        result
    }
}

pub fn dotted_to_relname(input: &str) -> TokenStream {
    let mut it = input.split('.');
    match (it.next(), it.next()) {
        (Some(module), Some(name)) => {
            let module = Ident::new(module, Span::call_site());
            let ident = Ident::new(&snake_to_camel(name), Span::call_site());
            quote::quote!(super::#module::#ident)
        }
        (Some(name), None) => {
            Ident::new(&snake_to_camel(name), Span::call_site()).into_token_stream()
        }
        _ => unreachable!(),
    }
}
