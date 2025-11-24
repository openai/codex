use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident};

use crate::helpers::{non_enum_error, HasStrumVariantProperties, HasTypeProperties};

pub fn enum_iter_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let gen = &ast.generics;
    let (impl_generics, ty_generics, where_clause) = gen.split_for_impl();
    let vis = &ast.vis;
    let type_properties = ast.get_type_properties()?;
    let strum_module_path = type_properties.crate_module_path();
    let doc_comment = format!("An iterator over the variants of [{}]", name);

    if gen.lifetimes().count() > 0 {
        return Err(syn::Error::new(
            Span::call_site(),
            "This macro doesn't support enums with lifetimes. \
             The resulting enums would be unbounded.",
        ));
    }

    let phantom_data = if gen.type_params().count() > 0 {
        let g = gen.type_params().map(|param| &param.ident);
        quote! { < ( #(#g),* ) > }
    } else {
        quote! { < () > }
    };

    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => return Err(non_enum_error()),
    };

    let mut arms = Vec::new();
    let mut idx = 0usize;
    for variant in variants {
        if variant.get_variant_properties()?.disabled.is_some() {
            continue;
        }

        let ident = &variant.ident;
        let params = match &variant.fields {
            Fields::Unit => quote! {},
            Fields::Unnamed(fields) => {
                let defaults = ::core::iter::repeat(quote!(::core::default::Default::default()))
                    .take(fields.unnamed.len());
                quote! { (#(#defaults),*) }
            }
            Fields::Named(fields) => {
                let fields = fields
                    .named
                    .iter()
                    .map(|field| field.ident.as_ref().unwrap());
                quote! { {#(#fields: ::core::default::Default::default()),*} }
            }
        };

        arms.push(quote! {#idx => ::core::option::Option::Some(#name::#ident #params)});
        idx += 1;
    }

    let variant_count = arms.len();
    arms.push(quote! { _ => ::core::option::Option::None });
    let iter_name = syn::parse_str::<Ident>(&format!("{}Iter", name)).unwrap();

    // Create a string literal "MyEnumIter" to use in the debug impl.
    let iter_name_debug_struct =
        syn::parse_str::<syn::LitStr>(&format!("\"{}\"", iter_name)).unwrap();

    Ok(quote! {
        #[doc = #doc_comment]
        #[allow(
            missing_copy_implementations,
        )]
        #vis struct #iter_name #impl_generics {
            idx: usize,
            back_idx: usize,
            marker: ::core::marker::PhantomData #phantom_data,
        }

        impl #impl_generics ::core::fmt::Debug for #iter_name #ty_generics #where_clause {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                // We don't know if the variants implement debug themselves so the only thing we
                // can really show is how many elements are left.
                f.debug_struct(#iter_name_debug_struct)
                    .field("len", &self.len())
                    .finish()
            }
        }

        impl #impl_generics #iter_name #ty_generics #where_clause {
            fn get(&self, idx: usize) -> ::core::option::Option<#name #ty_generics> {
                match idx {
                    #(#arms),*
                }
            }
        }

        impl #impl_generics #strum_module_path::IntoEnumIterator for #name #ty_generics #where_clause {
            type Iterator = #iter_name #ty_generics;
            fn iter() -> #iter_name #ty_generics {
                #iter_name {
                    idx: 0,
                    back_idx: 0,
                    marker: ::core::marker::PhantomData,
                }
            }
        }

        impl #impl_generics Iterator for #iter_name #ty_generics #where_clause {
            type Item = #name #ty_generics;

            fn next(&mut self) -> ::core::option::Option<<Self as Iterator>::Item> {
                self.nth(0)
            }

            fn size_hint(&self) -> (usize, ::core::option::Option<usize>) {
                let t = if self.idx + self.back_idx >= #variant_count { 0 } else { #variant_count - self.idx - self.back_idx };
                (t, Some(t))
            }

            fn nth(&mut self, n: usize) -> ::core::option::Option<<Self as Iterator>::Item> {
                let idx = self.idx + n + 1;
                if idx + self.back_idx > #variant_count {
                    // We went past the end of the iterator. Freeze idx at #variant_count
                    // so that it doesn't overflow if the user calls this repeatedly.
                    // See PR #76 for context.
                    self.idx = #variant_count;
                    ::core::option::Option::None
                } else {
                    self.idx = idx;
                    #iter_name::get(self, idx - 1)
                }
            }
        }

        impl #impl_generics ExactSizeIterator for #iter_name #ty_generics #where_clause {
            fn len(&self) -> usize {
                self.size_hint().0
            }
        }

        impl #impl_generics DoubleEndedIterator for #iter_name #ty_generics #where_clause {
            fn next_back(&mut self) -> ::core::option::Option<<Self as Iterator>::Item> {
                let back_idx = self.back_idx + 1;

                if self.idx + back_idx > #variant_count {
                    // We went past the end of the iterator. Freeze back_idx at #variant_count
                    // so that it doesn't overflow if the user calls this repeatedly.
                    // See PR #76 for context.
                    self.back_idx = #variant_count;
                    ::core::option::Option::None
                } else {
                    self.back_idx = back_idx;
                    #iter_name::get(self, #variant_count - self.back_idx)
                }
            }
        }

        impl #impl_generics ::core::iter::FusedIterator for #iter_name #ty_generics #where_clause { }

        impl #impl_generics Clone for #iter_name #ty_generics #where_clause {
            fn clone(&self) -> #iter_name #ty_generics {
                #iter_name {
                    idx: self.idx,
                    back_idx: self.back_idx,
                    marker: self.marker.clone(),
                }
            }
        }
    })
}
