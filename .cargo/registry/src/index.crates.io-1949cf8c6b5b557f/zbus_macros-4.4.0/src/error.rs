use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{spanned::Spanned, Data, DeriveInput, Error, Fields, Ident, Variant};
use zvariant_utils::def_attrs;

def_attrs! {
    crate zbus;

    pub StructAttributes("struct") {
        prefix str,
        impl_display bool
    };

    pub VariantAttributes("enum variant") {
        name str,
        error none
    };
}

use crate::utils::*;

pub fn expand_derive(input: DeriveInput) -> Result<TokenStream, Error> {
    let StructAttributes {
        prefix,
        impl_display,
    } = StructAttributes::parse(&input.attrs)?;
    let prefix = prefix.unwrap_or_else(|| "org.freedesktop.DBus".to_string());
    let generate_display = impl_display.unwrap_or(true);

    let (_vis, name, _generics, data) = match input.data {
        Data::Enum(data) => (input.vis, input.ident, input.generics, data),
        _ => return Err(Error::new(input.span(), "only enums supported")),
    };

    let zbus = zbus_path();
    let mut replies = quote! {};
    let mut error_names = quote! {};
    let mut error_descriptions = quote! {};
    let mut error_converts = quote! {};

    let mut zbus_error_variant = None;

    for variant in data.variants {
        let VariantAttributes { name, error } = VariantAttributes::parse(&variant.attrs)?;
        let ident = &variant.ident;
        let name = name.unwrap_or_else(|| ident.to_string());

        let fqn = if !error {
            format!("{prefix}.{name}")
        } else {
            // The ZBus error variant will always be a hardcoded string.
            String::from("org.freedesktop.zbus.Error")
        };

        let error_name = quote! {
            #zbus::names::ErrorName::from_static_str_unchecked(#fqn)
        };
        let e = match variant.fields {
            Fields::Unit => quote! {
                Self::#ident => #error_name,
            },
            Fields::Unnamed(_) => quote! {
                Self::#ident(..) => #error_name,
            },
            Fields::Named(_) => quote! {
                Self::#ident { .. } => #error_name,
            },
        };
        error_names.extend(e);

        if error {
            if zbus_error_variant.is_some() {
                panic!("More than 1 `#[zbus(error)]` variant found");
            }

            zbus_error_variant = Some(quote! { #ident });
        }

        // FIXME: this will error if the first field is not a string as per the dbus spec, but we
        // may support other cases?
        let e = match &variant.fields {
            Fields::Unit => quote! {
                Self::#ident => None,
            },
            Fields::Unnamed(_) => {
                if error {
                    quote! {
                        Self::#ident(#zbus::Error::MethodError(_, desc, _)) => desc.as_deref(),
                        Self::#ident(_) => None,
                    }
                } else {
                    quote! {
                        Self::#ident(desc, ..) => Some(&desc),
                    }
                }
            }
            Fields::Named(n) => {
                let f = &n
                    .named
                    .first()
                    .ok_or_else(|| Error::new(n.span(), "expected at least one field"))?
                    .ident;
                quote! {
                    Self::#ident { #f, } => Some(#f),
                }
            }
        };
        error_descriptions.extend(e);

        // The conversion for #[zbus(error)] variant is handled separately/explicitly.
        if !error {
            // FIXME: deserialize msg to error field instead, to support variable args
            let e = match &variant.fields {
                Fields::Unit => quote! {
                    #fqn => Self::#ident,
                },
                Fields::Unnamed(_) => quote! {
                    #fqn => { Self::#ident(::std::clone::Clone::clone(desc).unwrap_or_default()) },
                },
                Fields::Named(n) => {
                    let f = &n
                        .named
                        .first()
                        .ok_or_else(|| Error::new(n.span(), "expected at least one field"))?
                        .ident;
                    quote! {
                        #fqn => {
                            let desc = ::std::clone::Clone::clone(desc).unwrap_or_default();

                            Self::#ident { #f: desc }
                        }
                    }
                }
            };
            error_converts.extend(e);
        }

        let r = gen_reply_for_variant(&variant, error)?;
        replies.extend(r);
    }

    let from_zbus_error_impl = zbus_error_variant
        .map(|ident| {
            quote! {
                impl ::std::convert::From<#zbus::Error> for #name {
                    fn from(value: #zbus::Error) -> #name {
                        if let #zbus::Error::MethodError(name, desc, _) = &value {
                            match name.as_str() {
                                #error_converts
                                _ => Self::#ident(value),
                            }
                        } else {
                            Self::#ident(value)
                        }
                    }
                }
            }
        })
        .unwrap_or_default();

    let display_impl = if generate_display {
        quote! {
            impl ::std::fmt::Display for #name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    let name = #zbus::DBusError::name(self);
                    let description = #zbus::DBusError::description(self).unwrap_or("no description");
                    ::std::write!(f, "{}: {}", name, description)
                }
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        impl #zbus::DBusError for #name {
            fn name(&self) -> #zbus::names::ErrorName {
                match self {
                    #error_names
                }
            }

            fn description(&self) -> Option<&str> {
                match self {
                    #error_descriptions
                }
            }

            fn create_reply(&self, call: &#zbus::message::Header) -> #zbus::Result<#zbus::message::Message> {
                let name = self.name();
                match self {
                    #replies
                }
            }
        }

        #display_impl

        impl ::std::error::Error for #name {}

        #from_zbus_error_impl
    })
}

fn gen_reply_for_variant(
    variant: &Variant,
    zbus_error_variant: bool,
) -> Result<TokenStream, Error> {
    let zbus = zbus_path();
    let ident = &variant.ident;
    match &variant.fields {
        Fields::Unit => Ok(quote! {
            Self::#ident => #zbus::message::Builder::error(call, name)?.build(&()),
        }),
        Fields::Unnamed(f) => {
            // Name the unnamed fields as the number of the field with an 'f' in front.
            let in_fields = (0..f.unnamed.len())
                .map(|n| Ident::new(&format!("f{n}"), ident.span()).to_token_stream())
                .collect::<Vec<_>>();
            let out_fields = if zbus_error_variant {
                let error_field = in_fields.first().ok_or_else(|| {
                    Error::new(
                        ident.span(),
                        "expected at least one field for #[zbus(error)] variant",
                    )
                })?;
                vec![quote! {
                    match #error_field {
                        #zbus::Error::MethodError(name, desc, _) => {
                            ::std::clone::Clone::clone(desc)
                        }
                        _ => None,
                    }
                    .unwrap_or_else(|| ::std::string::ToString::to_string(#error_field))
                }]
            } else {
                // FIXME: Workaround for https://github.com/rust-lang/rust-clippy/issues/10577
                #[allow(clippy::redundant_clone)]
                in_fields.clone()
            };

            Ok(quote! {
                Self::#ident(#(#in_fields),*) => #zbus::message::Builder::error(call, name)?.build(&(#(#out_fields),*)),
            })
        }
        Fields::Named(f) => {
            let fields = f.named.iter().map(|v| v.ident.as_ref()).collect::<Vec<_>>();
            Ok(quote! {
                Self::#ident { #(#fields),* } => #zbus::message::Builder::error(call, name)?.build(&(#(#fields),*)),
            })
        }
    }
}
