use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{Attribute, Expr, ExprLit, Lit};

pub fn get_doc(attrs: &[Attribute]) -> Option<Expr> {
    let mut macro_args: TokenStream = TokenStream::new();

    for (i, line) in attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .flat_map(|a| a.meta.require_name_value())
        .enumerate()
    {
        if i > 0 {
            macro_args.extend([quote!(, "\n",)]);
        }

        if let Expr::Lit(ExprLit {
            lit: Lit::Str(lit_str),
            ..
        }) = &line.value
        {
            if let Some(trimmed) = lit_str.value().strip_prefix(' ') {
                trimmed.to_tokens(&mut macro_args);
                continue;
            }
        }

        line.value.to_tokens(&mut macro_args);
    }

    if macro_args.is_empty() {
        None
    } else {
        Some(parse_quote!(::core::concat!(#macro_args)))
    }
}
