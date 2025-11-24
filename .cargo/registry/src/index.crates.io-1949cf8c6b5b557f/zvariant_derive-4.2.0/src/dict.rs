use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{punctuated::Punctuated, spanned::Spanned, Data, DeriveInput, Error, Field};
use zvariant_utils::{case, macros};

use crate::utils::*;

fn dict_name_for_field(
    f: &Field,
    rename_attr: Option<String>,
    rename_all_attr: Option<&str>,
) -> Result<String, Error> {
    if let Some(name) = rename_attr {
        Ok(name)
    } else {
        let ident = f.ident.as_ref().unwrap().to_string();

        match rename_all_attr {
            Some("lowercase") => Ok(ident.to_ascii_lowercase()),
            Some("UPPERCASE") => Ok(ident.to_ascii_uppercase()),
            Some("PascalCase") => Ok(case::pascal_or_camel_case(&ident, true)),
            Some("camelCase") => Ok(case::pascal_or_camel_case(&ident, false)),
            Some("snake_case") => Ok(case::snake_or_kebab_case(&ident, true)),
            Some("kebab-case") => Ok(case::snake_or_kebab_case(&ident, false)),
            None => Ok(ident),
            Some(other) => Err(Error::new(
                f.span(),
                format!("invalid `rename_all` attribute value {other}"),
            )),
        }
    }
}

pub fn expand_serialize_derive(input: DeriveInput) -> Result<TokenStream, Error> {
    let (name, data) = match input.data {
        Data::Struct(data) => (input.ident, data),
        _ => return Err(Error::new(input.span(), "only structs supported")),
    };

    let StructAttributes { rename_all, .. } = StructAttributes::parse(&input.attrs)?;

    let zv = zvariant_path();
    let mut entries = quote! {};
    let mut num_entries: usize = 0;

    for f in &data.fields {
        let FieldAttributes { rename } = FieldAttributes::parse(&f.attrs)?;

        let name = &f.ident;
        let dict_name = dict_name_for_field(f, rename, rename_all.as_deref())?;

        let is_option = macros::ty_is_option(&f.ty);

        let e = if is_option {
            quote! {
                if self.#name.is_some() {
                    map.serialize_entry(#dict_name, &#zv::SerializeValue(self.#name.as_ref().unwrap()))?;
                }
            }
        } else {
            quote! {
                map.serialize_entry(#dict_name, &#zv::SerializeValue(&self.#name))?;
            }
        };

        entries.extend(e);
        num_entries += 1;
    }

    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let num_entries = num_entries.to_token_stream();
    Ok(quote! {
        #[allow(deprecated)]
        impl #impl_generics #zv::export::serde::ser::Serialize for #name #ty_generics
        #where_clause
        {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: #zv::export::serde::ser::Serializer,
            {
                use #zv::export::serde::ser::SerializeMap;

                // zbus doesn't care about number of entries (it would need bytes instead)
                let mut map = serializer.serialize_map(::std::option::Option::Some(#num_entries))?;
                #entries
                map.end()
            }
        }
    })
}

pub fn expand_deserialize_derive(input: DeriveInput) -> Result<TokenStream, Error> {
    let (name, data) = match input.data {
        Data::Struct(data) => (input.ident, data),
        _ => return Err(Error::new(input.span(), "only structs supported")),
    };

    let StructAttributes {
        rename_all,
        deny_unknown_fields,
        ..
    } = StructAttributes::parse(&input.attrs)?;

    let visitor = format_ident!("{}Visitor", name);
    let zv = zvariant_path();
    let mut fields = Vec::new();
    let mut req_fields = Vec::new();
    let mut dict_names = Vec::new();
    let mut entries = Vec::new();

    for f in &data.fields {
        let FieldAttributes { rename } = FieldAttributes::parse(&f.attrs)?;

        let name = &f.ident;
        let dict_name = dict_name_for_field(f, rename, rename_all.as_deref())?;

        let is_option = macros::ty_is_option(&f.ty);

        entries.push(quote! {
            #dict_name => {
                // FIXME: add an option about strict parsing (instead of silently skipping the field)
                #name = access.next_value::<#zv::DeserializeValue<_>>().map(|v| v.0).ok();
            }
        });

        dict_names.push(dict_name);
        fields.push(name);

        if !is_option {
            req_fields.push(name);
        }
    }

    let fallback = if deny_unknown_fields {
        quote! {
            field => {
                return ::std::result::Result::Err(
                    <M::Error as #zv::export::serde::de::Error>::unknown_field(
                        field,
                        &[#(#dict_names),*],
                    ),
                );
            }
        }
    } else {
        quote! {
            unknown => {
                let _ = access.next_value::<#zv::Value>();
            }
        }
    };
    entries.push(fallback);

    let (_, ty_generics, _) = input.generics.split_for_impl();
    let mut generics = input.generics.clone();
    let def = syn::LifetimeParam {
        attrs: Vec::new(),
        lifetime: syn::Lifetime::new("'de", Span::call_site()),
        colon_token: None,
        bounds: Punctuated::new(),
    };
    generics.params = Some(syn::GenericParam::Lifetime(def))
        .into_iter()
        .chain(generics.params)
        .collect();

    let (impl_generics, _, where_clause) = generics.split_for_impl();

    Ok(quote! {
        #[allow(deprecated)]
        impl #impl_generics #zv::export::serde::de::Deserialize<'de> for #name #ty_generics
        #where_clause
        {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: #zv::export::serde::de::Deserializer<'de>,
            {
                struct #visitor #ty_generics(::std::marker::PhantomData<#name #ty_generics>);

                impl #impl_generics #zv::export::serde::de::Visitor<'de> for #visitor #ty_generics {
                    type Value = #name #ty_generics;

                    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                        formatter.write_str("a dictionary")
                    }

                    fn visit_map<M>(
                        self,
                        mut access: M,
                    ) -> ::std::result::Result<Self::Value, M::Error>
                    where
                        M: #zv::export::serde::de::MapAccess<'de>,
                    {
                        #( let mut #fields = ::std::default::Default::default(); )*

                        // does not check duplicated fields, since those shouldn't exist in stream
                        while let ::std::option::Option::Some(key) = access.next_key::<&str>()? {
                            match key {
                                #(#entries)*
                            }
                        }

                        #(let #req_fields = if let ::std::option::Option::Some(val) = #req_fields {
                            val
                        } else {
                            return ::std::result::Result::Err(
                                <M::Error as #zv::export::serde::de::Error>::missing_field(
                                    ::std::stringify!(#req_fields),
                                ),
                            );
                        };)*

                        ::std::result::Result::Ok(#name { #(#fields),* })
                    }
                }


                deserializer.deserialize_map(#visitor(::std::marker::PhantomData))
            }
        }
    })
}
