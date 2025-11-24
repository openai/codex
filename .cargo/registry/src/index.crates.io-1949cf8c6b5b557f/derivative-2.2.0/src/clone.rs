use proc_macro2;

use ast;
use attr;
use matcher;
use syn;
use utils;

/// Derive `Copy` for `input`.
pub fn derive_copy(input: &ast::Input) -> proc_macro2::TokenStream {
    let name = &input.ident;

    let copy_trait_path = copy_trait_path();
    let generics = utils::build_impl_generics(
        input,
        &copy_trait_path,
        |attrs| attrs.copy_bound().is_none(),
        |field| field.copy_bound(),
        |input| input.copy_bound(),
    );
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        #[allow(unused_qualifications)]
        impl #impl_generics #copy_trait_path for #name #ty_generics #where_clause {}
    }
}

/// Derive `Clone` for `input`.
pub fn derive_clone(input: &ast::Input) -> proc_macro2::TokenStream {
    let name = &input.ident;

    let clone_trait_path = clone_trait_path();
    let generics = utils::build_impl_generics(
        input,
        &clone_trait_path,
        needs_clone_bound,
        |field| field.clone_bound(),
        |input| input.clone_bound(),
    );
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let is_copy = input.attrs.copy.is_some();
    if is_copy && input.generics.type_params().count() == 0 {
        quote! {
            #[allow(unused_qualifications)]
            impl #impl_generics #clone_trait_path for #name #ty_generics #where_clause {
                fn clone(&self) -> Self {
                    *self
                }
            }
        }
    } else {
        let body = matcher::Matcher::new(matcher::BindingStyle::Ref, input.attrs.is_packed).build_arms(
            input,
            "__arg",
            |arm_path, _, _, style, _, bis| {
                let field_clones = bis.iter().map(|bi| {
                    let arg = &bi.expr;

                    let clone = if let Some(clone_with) = bi.field.attrs.clone_with() {
                        quote!(#clone_with(&#arg))
                    } else {
                        quote!(#arg.clone())
                    };

                    if let Some(ref name) = bi.field.ident {
                        quote! {
                            #name: #clone
                        }
                    } else {
                        clone
                    }
                });

                match style {
                    ast::Style::Struct => {
                        quote! {
                            #arm_path {
                                #(#field_clones),*
                            }
                        }
                    }
                    ast::Style::Tuple => {
                        quote! {
                            #arm_path (#(#field_clones),*)
                        }
                    }
                    ast::Style::Unit => {
                        quote! {
                            #arm_path
                        }
                    }
                }
            },
        );

        let clone_from = if input.attrs.clone_from() {
            Some(
                matcher::Matcher::new(matcher::BindingStyle::RefMut, input.attrs.is_packed).build_arms(
                    input,
                    "__arg",
                    |outer_arm_path, _, _, _, _, outer_bis| {
                        let body = matcher::Matcher::new(matcher::BindingStyle::Ref, input.attrs.is_packed).build_arms(
                            input,
                            "__other",
                            |inner_arm_path, _, _, _, _, inner_bis| {
                                if outer_arm_path == inner_arm_path {
                                    let field_clones = outer_bis.iter().zip(inner_bis).map(
                                        |(outer_bi, inner_bi)| {
                                            let outer = &outer_bi.expr;
                                            let inner = &inner_bi.expr;

                                            quote!(#outer.clone_from(&#inner);)
                                        },
                                    );

                                    quote! {
                                        #(#field_clones)*
                                        return;
                                    }
                                } else {
                                    quote!()
                                }
                            },
                        );

                        quote! {
                            match *other {
                                #body
                            }
                        }
                    },
                ),
            )
        } else {
            None
        };

        let clone_from = clone_from.map(|body| {
            // Enumerations are only cloned-from if both variants are the same.
            // If they are different, fallback to normal cloning.
            let fallback = if let ast::Body::Enum(_) = input.body {
                Some(quote!(*self = other.clone();))
            } else {
                None
            };

            quote! {
                #[allow(clippy::needless_return)]
                fn clone_from(&mut self, other: &Self) {
                    match *self {
                        #body
                    }

                    #fallback
                }
            }
        });

        quote! {
            #[allow(unused_qualifications)]
            impl #impl_generics #clone_trait_path for #name #ty_generics #where_clause {
                fn clone(&self) -> Self {
                    match *self {
                        #body
                    }
                }

                #clone_from
            }
        }
    }
}

fn needs_clone_bound(attrs: &attr::Field) -> bool {
    attrs.clone_bound().is_none()
}

/// Return the path of the `Clone` trait, that is `::std::clone::Clone`.
fn clone_trait_path() -> syn::Path {
    if cfg!(feature = "use_core") {
        parse_quote!(::core::clone::Clone)
    } else {
        parse_quote!(::std::clone::Clone)
    }
}

/// Return the path of the `Copy` trait, that is `::std::marker::Copy`.
fn copy_trait_path() -> syn::Path {
    if cfg!(feature = "use_core") {
        parse_quote!(::core::marker::Copy)
    } else {
        parse_quote!(::std::marker::Copy)
    }
}
