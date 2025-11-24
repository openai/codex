use proc_macro2::TokenStream;
use quote::quote;
use syn::{Fields, Ident, Variant};

pub mod as_ref_str;
pub mod display;
pub mod from_string;
pub mod to_string;

struct NonSingleFieldEnum;

fn extract_single_field_variant_and_then<F>(
    name: &Ident,
    variant: &Variant,
    return_val_fn: F,
) -> Result<TokenStream, NonSingleFieldEnum>
where
    F: Fn(&TokenStream) -> TokenStream,
{
    let variant_ident = &variant.ident;

    let pattern_and_return = match &variant.fields {
        Fields::Unnamed(f) if f.unnamed.len() == 1 => {
            let ident = &quote! { field0 };
            let ref_kw = match f.unnamed.last().unwrap().ty {
                syn::Type::Reference(..) => quote! {},
                _ => quote! { ref },
            };

            let ret_val = return_val_fn(ident);
            quote! { (#ref_kw #ident) => #ret_val }
        }
        Fields::Named(f) if f.named.len() == 1 => {
            let field = f.named.last().unwrap();
            let ref_kw = match field.ty {
                syn::Type::Reference(..) => quote! {},
                _ => quote! { ref },
            };

            let ident = field.ident.as_ref().unwrap();
            let ident = &quote! { #ident };
            let ret_val = return_val_fn(ident);
            quote! { { #ref_kw #ident} => #ret_val }
        }
        _ => return Err(NonSingleFieldEnum),
    };

    Ok(quote! { #name::#variant_ident #pattern_and_return })
}
