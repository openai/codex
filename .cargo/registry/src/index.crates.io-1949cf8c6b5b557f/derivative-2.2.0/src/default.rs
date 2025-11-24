use proc_macro2;

use ast;
use attr;
use syn;
use utils;

/// Derive `Default` for `input`.
pub fn derive(input: &ast::Input, default: &attr::InputDefault) -> proc_macro2::TokenStream {
    fn make_variant_data(
        variant_name: &proc_macro2::TokenStream,
        style: ast::Style,
        fields: &[ast::Field],
    ) -> proc_macro2::TokenStream {
        let default_trait_path = default_trait_path();

        match style {
            ast::Style::Struct => {
                let mut defaults = Vec::new();

                for f in fields {
                    let name = f
                        .ident
                        .as_ref()
                        .expect("A structure field must have a name");
                    let default = f
                        .attrs
                        .default_value()
                        .map_or_else(|| quote!(#default_trait_path::default()), |v| quote!(#v));

                    defaults.push(quote!(#name: #default));
                }

                quote!(#variant_name { #(#defaults),* })
            }
            ast::Style::Tuple => {
                let mut defaults = Vec::new();

                for f in fields {
                    let default = f
                        .attrs
                        .default_value()
                        .map_or_else(|| quote!(#default_trait_path::default()), |v| quote!(#v));

                    defaults.push(default);
                }

                quote!(#variant_name ( #(#defaults),* ))
            }
            ast::Style::Unit => quote!(#variant_name),
        }
    }

    let name = &input.ident;
    let default_trait_path = default_trait_path();
    let generics = utils::build_impl_generics(
        input,
        &default_trait_path,
        |attrs| attrs.default_bound().is_none(),
        |field| field.default_bound(),
        |input| input.default_bound(),
    );
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let body = match input.body {
        ast::Body::Enum(ref data) => {
            let arms = data.iter().filter_map(|variant| {
                if variant.attrs.default.is_some() {
                    let vname = &variant.ident;

                    Some(make_variant_data(
                        &quote!(#name::#vname),
                        variant.style,
                        &variant.fields,
                    ))
                } else {
                    None
                }
            });

            quote!(#(#arms),*)
        }
        ast::Body::Struct(style, ref vd) => make_variant_data(&quote!(#name), style, vd),
    };

    let new_fn = if default.new {
        Some(quote!(
            #[allow(unused_qualifications)]
            impl #impl_generics #name #ty_generics #where_clause {
                /// Creates a default value for this type.
                #[inline]
                pub fn new() -> Self {
                    <Self as #default_trait_path>::default()
                }
            }
        ))
    } else {
        None
    };

    quote!(
        #new_fn

        #[allow(unused_qualifications)]
        impl #impl_generics #default_trait_path for #name #ty_generics #where_clause {
            fn default() -> Self {
                #body
            }
        }
    )
}

/// Return the path of the `Default` trait, that is `::std::default::Default`.
fn default_trait_path() -> syn::Path {
    if cfg!(feature = "use_core") {
        parse_quote!(::core::default::Default)
    } else {
        parse_quote!(::std::default::Default)
    }
}
