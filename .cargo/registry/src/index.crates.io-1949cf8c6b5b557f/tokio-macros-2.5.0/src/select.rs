use proc_macro::{TokenStream, TokenTree};
use proc_macro2::Span;
use quote::quote;
use syn::{parse::Parser, Ident};

pub(crate) fn declare_output_enum(input: TokenStream) -> TokenStream {
    // passed in is: `(_ _ _)` with one `_` per branch
    let branches = match input.into_iter().next() {
        Some(TokenTree::Group(group)) => group.stream().into_iter().count(),
        _ => panic!("unexpected macro input"),
    };

    let variants = (0..branches)
        .map(|num| Ident::new(&format!("_{num}"), Span::call_site()))
        .collect::<Vec<_>>();

    // Use a bitfield to track which futures completed
    let mask = Ident::new(
        if branches <= 8 {
            "u8"
        } else if branches <= 16 {
            "u16"
        } else if branches <= 32 {
            "u32"
        } else if branches <= 64 {
            "u64"
        } else {
            panic!("up to 64 branches supported");
        },
        Span::call_site(),
    );

    TokenStream::from(quote! {
        pub(super) enum Out<#( #variants ),*> {
            #( #variants(#variants), )*
            // Include a `Disabled` variant signifying that all select branches
            // failed to resolve.
            Disabled,
        }

        pub(super) type Mask = #mask;
    })
}

pub(crate) fn clean_pattern_macro(input: TokenStream) -> TokenStream {
    // If this isn't a pattern, we return the token stream as-is. The select!
    // macro is using it in a location requiring a pattern, so an error will be
    // emitted there.
    let mut input: syn::Pat = match syn::Pat::parse_single.parse(input.clone()) {
        Ok(it) => it,
        Err(_) => return input,
    };

    clean_pattern(&mut input);
    quote::ToTokens::into_token_stream(input).into()
}

// Removes any occurrences of ref or mut in the provided pattern.
fn clean_pattern(pat: &mut syn::Pat) {
    match pat {
        syn::Pat::Lit(_literal) => {}
        syn::Pat::Macro(_macro) => {}
        syn::Pat::Path(_path) => {}
        syn::Pat::Range(_range) => {}
        syn::Pat::Rest(_rest) => {}
        syn::Pat::Verbatim(_tokens) => {}
        syn::Pat::Wild(_underscore) => {}
        syn::Pat::Ident(ident) => {
            ident.by_ref = None;
            ident.mutability = None;
            if let Some((_at, pat)) = &mut ident.subpat {
                clean_pattern(&mut *pat);
            }
        }
        syn::Pat::Or(or) => {
            for case in &mut or.cases {
                clean_pattern(case);
            }
        }
        syn::Pat::Slice(slice) => {
            for elem in &mut slice.elems {
                clean_pattern(elem);
            }
        }
        syn::Pat::Struct(struct_pat) => {
            for field in &mut struct_pat.fields {
                clean_pattern(&mut field.pat);
            }
        }
        syn::Pat::Tuple(tuple) => {
            for elem in &mut tuple.elems {
                clean_pattern(elem);
            }
        }
        syn::Pat::TupleStruct(tuple) => {
            for elem in &mut tuple.elems {
                clean_pattern(elem);
            }
        }
        syn::Pat::Reference(reference) => {
            reference.mutability = None;
            clean_pattern(&mut reference.pat);
        }
        syn::Pat::Type(type_pat) => {
            clean_pattern(&mut type_pat.pat);
        }
        _ => {}
    }
}
