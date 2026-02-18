use proc_macro::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::Attribute;
use syn::Data;
use syn::DeriveInput;
use syn::Fields;
use syn::Type;
use syn::parse_macro_input;

#[proc_macro_derive(ModelPatch, attributes(model_patch))]
pub fn derive_model_patch(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_model_patch_impl(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn derive_model_patch_impl(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "ModelPatch does not support generics",
        ));
    }

    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "ModelPatch only supports structs",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "ModelPatch only supports structs with named fields",
        ));
    };

    let source_name = &input.ident;
    let patch_name = format_ident!("{source_name}Patch");
    let visibility = &input.vis;
    let mut patch_fields = Vec::new();
    let mut patch_apply_logic = Vec::new();

    for field in &fields.named {
        let Some(field_name) = &field.ident else {
            return Err(syn::Error::new_spanned(
                field,
                "ModelPatch requires named fields",
            ));
        };
        if has_model_patch_skip(&field.attrs)? {
            continue;
        }

        let field_type = &field.ty;
        let patch_field_inner_type = option_inner(field_type)
            .cloned()
            .unwrap_or_else(|| field_type.clone());
        patch_fields.push(quote! {
            pub #field_name: ::std::option::Option<#patch_field_inner_type>,
        });

        if option_inner(field_type).is_some() {
            patch_apply_logic.push(quote! {
                if let ::std::option::Option::Some(value) = self.#field_name.as_ref() {
                    target.#field_name = ::std::option::Option::Some(value.clone());
                }
            });
        } else {
            patch_apply_logic.push(quote! {
                if let ::std::option::Option::Some(value) = self.#field_name.as_ref() {
                    target.#field_name = value.clone();
                }
            });
        }
    }

    Ok(quote! {
        #[derive(
            ::std::fmt::Debug,
            ::serde::Serialize,
            ::serde::Deserialize,
            ::std::clone::Clone,
            ::std::default::Default,
            ::std::cmp::PartialEq,
            ::ts_rs::TS,
            ::schemars::JsonSchema
        )]
        #[serde(default, deny_unknown_fields)]
        #visibility struct #patch_name {
            #(#patch_fields)*
        }

        impl #patch_name {
            pub fn apply_to(&self, target: &mut #source_name) {
                #(#patch_apply_logic)*
            }
        }
    })
}

fn has_model_patch_skip(attrs: &[Attribute]) -> syn::Result<bool> {
    let mut skip = false;
    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("model_patch"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                skip = true;
                return Ok(());
            }
            Err(meta.error("unsupported model_patch attribute; expected `skip`"))
        })?;
    }
    Ok(skip)
}

fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    })
}
