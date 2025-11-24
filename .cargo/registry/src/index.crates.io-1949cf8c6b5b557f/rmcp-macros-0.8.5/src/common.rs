//! Common utilities shared between different macro implementations

use quote::quote;
use syn::{Attribute, Expr, FnArg, ImplItemFn, Signature, Type};

/// Parse a None expression
pub fn none_expr() -> syn::Result<Expr> {
    syn::parse2::<Expr>(quote! { None })
}

/// Extract documentation from doc attributes
pub fn extract_doc_line(
    existing_docs: Option<Expr>,
    attr: &Attribute,
) -> syn::Result<Option<Expr>> {
    if !attr.path().is_ident("doc") {
        return Ok(None);
    }

    let syn::Meta::NameValue(name_value) = &attr.meta else {
        return Ok(None);
    };

    let value = &name_value.value;
    let this_expr: Option<Expr> = match value {
        // Preserve macros such as `include_str!(...)`
        syn::Expr::Macro(_) => Some(value.clone()),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit_str),
            ..
        }) => {
            let content = lit_str.value().trim().to_string();
            if content.is_empty() {
                return Ok(existing_docs);
            }
            Some(Expr::Lit(syn::ExprLit {
                attrs: Vec::new(),
                lit: syn::Lit::Str(syn::LitStr::new(&content, lit_str.span())),
            }))
        }
        _ => return Ok(None),
    };

    match (existing_docs, this_expr) {
        (Some(existing), Some(this)) => {
            syn::parse2::<Expr>(quote! { concat!(#existing, "\n", #this) }).map(Some)
        }
        (Some(existing), None) => Ok(Some(existing)),
        (None, Some(this)) => Ok(Some(this)),
        _ => Ok(None),
    }
}

/// Find Parameters<T> type in function signature
/// Returns the full Parameters<T> type if found
pub fn find_parameters_type_in_sig(sig: &Signature) -> Option<Box<Type>> {
    sig.inputs.iter().find_map(|input| {
        if let FnArg::Typed(pat_type) = input {
            if let Type::Path(type_path) = &*pat_type.ty {
                if type_path
                    .path
                    .segments
                    .last()
                    .is_some_and(|type_name| type_name.ident == "Parameters")
                {
                    return Some(pat_type.ty.clone());
                }
            }
        }
        None
    })
}

/// Find Parameters<T> type in ImplItemFn
pub fn find_parameters_type_impl(fn_item: &ImplItemFn) -> Option<Box<Type>> {
    find_parameters_type_in_sig(&fn_item.sig)
}
