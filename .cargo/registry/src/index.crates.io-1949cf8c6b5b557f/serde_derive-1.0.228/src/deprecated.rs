use proc_macro2::TokenStream;
use quote::quote;

pub fn allow_deprecated(input: &syn::DeriveInput) -> Option<TokenStream> {
    if should_allow_deprecated(input) {
        Some(quote! { #[allow(deprecated)] })
    } else {
        None
    }
}

/// Determine if an `#[allow(deprecated)]` should be added to the derived impl.
///
/// This should happen if the derive input or an enum variant it contains has
/// one of:
///   - `#[deprecated]`
///   - `#[allow(deprecated)]`
fn should_allow_deprecated(input: &syn::DeriveInput) -> bool {
    if contains_deprecated(&input.attrs) {
        return true;
    }
    if let syn::Data::Enum(data_enum) = &input.data {
        for variant in &data_enum.variants {
            if contains_deprecated(&variant.attrs) {
                return true;
            }
        }
    }
    false
}

/// Check whether the given attributes contains one of:
///   - `#[deprecated]`
///   - `#[allow(deprecated)]`
fn contains_deprecated(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("deprecated") {
            return true;
        }
        if let syn::Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("allow") {
                let mut allow_deprecated = false;
                let _ = meta_list.parse_nested_meta(|meta| {
                    if meta.path.is_ident("deprecated") {
                        allow_deprecated = true;
                    }
                    Ok(())
                });
                if allow_deprecated {
                    return true;
                }
            }
        }
    }
    false
}
