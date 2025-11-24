use crate::ast::{Enum, Field, Input, Struct};
use crate::attr::Trait;
use crate::generics::InferredBounds;
use crate::span::MemberSpan;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned, ToTokens};
use std::collections::BTreeSet as Set;
use syn::{DeriveInput, GenericArgument, Member, PathArguments, Result, Token, Type};

pub fn derive(input: &DeriveInput) -> TokenStream {
    match try_expand(input) {
        Ok(expanded) => expanded,
        // If there are invalid attributes in the input, expand to an Error impl
        // anyway to minimize spurious knock-on errors in other code that uses
        // this type as an Error.
        Err(error) => fallback(input, error),
    }
}

fn try_expand(input: &DeriveInput) -> Result<TokenStream> {
    let input = Input::from_syn(input)?;
    input.validate()?;
    Ok(match input {
        Input::Struct(input) => impl_struct(input),
        Input::Enum(input) => impl_enum(input),
    })
}

fn fallback(input: &DeriveInput, error: syn::Error) -> TokenStream {
    let ty = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let error = error.to_compile_error();

    quote! {
        #error

        #[allow(unused_qualifications)]
        #[automatically_derived]
        impl #impl_generics std::error::Error for #ty #ty_generics #where_clause
        where
            // Work around trivial bounds being unstable.
            // https://github.com/rust-lang/rust/issues/48214
            for<'workaround> #ty #ty_generics: ::core::fmt::Debug,
        {}

        #[allow(unused_qualifications)]
        #[automatically_derived]
        impl #impl_generics ::core::fmt::Display for #ty #ty_generics #where_clause {
            fn fmt(&self, __formatter: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::unreachable!()
            }
        }
    }
}

fn impl_struct(input: Struct) -> TokenStream {
    let ty = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut error_inferred_bounds = InferredBounds::new();

    let source_body = if let Some(transparent_attr) = &input.attrs.transparent {
        let only_field = &input.fields[0];
        if only_field.contains_generic {
            error_inferred_bounds.insert(only_field.ty, quote!(std::error::Error));
        }
        let member = &only_field.member;
        Some(quote_spanned! {transparent_attr.span=>
            std::error::Error::source(self.#member.as_dyn_error())
        })
    } else if let Some(source_field) = input.source_field() {
        let source = &source_field.member;
        if source_field.contains_generic {
            let ty = unoptional_type(source_field.ty);
            error_inferred_bounds.insert(ty, quote!(std::error::Error + 'static));
        }
        let asref = if type_is_option(source_field.ty) {
            Some(quote_spanned!(source.member_span()=> .as_ref()?))
        } else {
            None
        };
        let dyn_error = quote_spanned! {source_field.source_span()=>
            self.#source #asref.as_dyn_error()
        };
        Some(quote! {
            ::core::option::Option::Some(#dyn_error)
        })
    } else {
        None
    };
    let source_method = source_body.map(|body| {
        quote! {
            fn source(&self) -> ::core::option::Option<&(dyn std::error::Error + 'static)> {
                use thiserror::__private::AsDynError as _;
                #body
            }
        }
    });

    let provide_method = input.backtrace_field().map(|backtrace_field| {
        let request = quote!(request);
        let backtrace = &backtrace_field.member;
        let body = if let Some(source_field) = input.source_field() {
            let source = &source_field.member;
            let source_provide = if type_is_option(source_field.ty) {
                quote_spanned! {source.member_span()=>
                    if let ::core::option::Option::Some(source) = &self.#source {
                        source.thiserror_provide(#request);
                    }
                }
            } else {
                quote_spanned! {source.member_span()=>
                    self.#source.thiserror_provide(#request);
                }
            };
            let self_provide = if source == backtrace {
                None
            } else if type_is_option(backtrace_field.ty) {
                Some(quote! {
                    if let ::core::option::Option::Some(backtrace) = &self.#backtrace {
                        #request.provide_ref::<std::backtrace::Backtrace>(backtrace);
                    }
                })
            } else {
                Some(quote! {
                    #request.provide_ref::<std::backtrace::Backtrace>(&self.#backtrace);
                })
            };
            quote! {
                use thiserror::__private::ThiserrorProvide as _;
                #source_provide
                #self_provide
            }
        } else if type_is_option(backtrace_field.ty) {
            quote! {
                if let ::core::option::Option::Some(backtrace) = &self.#backtrace {
                    #request.provide_ref::<std::backtrace::Backtrace>(backtrace);
                }
            }
        } else {
            quote! {
                #request.provide_ref::<std::backtrace::Backtrace>(&self.#backtrace);
            }
        };
        quote! {
            fn provide<'_request>(&'_request self, #request: &mut std::error::Request<'_request>) {
                #body
            }
        }
    });

    let mut display_implied_bounds = Set::new();
    let display_body = if input.attrs.transparent.is_some() {
        let only_field = &input.fields[0].member;
        display_implied_bounds.insert((0, Trait::Display));
        Some(quote! {
            ::core::fmt::Display::fmt(&self.#only_field, __formatter)
        })
    } else if let Some(display) = &input.attrs.display {
        display_implied_bounds.clone_from(&display.implied_bounds);
        let use_as_display = use_as_display(display.has_bonus_display);
        let pat = fields_pat(&input.fields);
        Some(quote! {
            #use_as_display
            #[allow(unused_variables, deprecated)]
            let Self #pat = self;
            #display
        })
    } else {
        None
    };
    let display_impl = display_body.map(|body| {
        let mut display_inferred_bounds = InferredBounds::new();
        for (field, bound) in display_implied_bounds {
            let field = &input.fields[field];
            if field.contains_generic {
                display_inferred_bounds.insert(field.ty, bound);
            }
        }
        let display_where_clause = display_inferred_bounds.augment_where_clause(input.generics);
        quote! {
            #[allow(unused_qualifications)]
            #[automatically_derived]
            impl #impl_generics ::core::fmt::Display for #ty #ty_generics #display_where_clause {
                #[allow(clippy::used_underscore_binding)]
                fn fmt(&self, __formatter: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    #body
                }
            }
        }
    });

    let from_impl = input.from_field().map(|from_field| {
        let backtrace_field = input.distinct_backtrace_field();
        let from = unoptional_type(from_field.ty);
        let body = from_initializer(from_field, backtrace_field);
        quote! {
            #[allow(unused_qualifications)]
            #[automatically_derived]
            impl #impl_generics ::core::convert::From<#from> for #ty #ty_generics #where_clause {
                #[allow(deprecated)]
                fn from(source: #from) -> Self {
                    #ty #body
                }
            }
        }
    });

    if input.generics.type_params().next().is_some() {
        let self_token = <Token![Self]>::default();
        error_inferred_bounds.insert(self_token, Trait::Debug);
        error_inferred_bounds.insert(self_token, Trait::Display);
    }
    let error_where_clause = error_inferred_bounds.augment_where_clause(input.generics);

    quote! {
        #[allow(unused_qualifications)]
        #[automatically_derived]
        impl #impl_generics std::error::Error for #ty #ty_generics #error_where_clause {
            #source_method
            #provide_method
        }
        #display_impl
        #from_impl
    }
}

fn impl_enum(input: Enum) -> TokenStream {
    let ty = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut error_inferred_bounds = InferredBounds::new();

    let source_method = if input.has_source() {
        let arms = input.variants.iter().map(|variant| {
            let ident = &variant.ident;
            if let Some(transparent_attr) = &variant.attrs.transparent {
                let only_field = &variant.fields[0];
                if only_field.contains_generic {
                    error_inferred_bounds.insert(only_field.ty, quote!(std::error::Error));
                }
                let member = &only_field.member;
                let source = quote_spanned! {transparent_attr.span=>
                    std::error::Error::source(transparent.as_dyn_error())
                };
                quote! {
                    #ty::#ident {#member: transparent} => #source,
                }
            } else if let Some(source_field) = variant.source_field() {
                let source = &source_field.member;
                if source_field.contains_generic {
                    let ty = unoptional_type(source_field.ty);
                    error_inferred_bounds.insert(ty, quote!(std::error::Error + 'static));
                }
                let asref = if type_is_option(source_field.ty) {
                    Some(quote_spanned!(source.member_span()=> .as_ref()?))
                } else {
                    None
                };
                let varsource = quote!(source);
                let dyn_error = quote_spanned! {source_field.source_span()=>
                    #varsource #asref.as_dyn_error()
                };
                quote! {
                    #ty::#ident {#source: #varsource, ..} => ::core::option::Option::Some(#dyn_error),
                }
            } else {
                quote! {
                    #ty::#ident {..} => ::core::option::Option::None,
                }
            }
        });
        Some(quote! {
            fn source(&self) -> ::core::option::Option<&(dyn std::error::Error + 'static)> {
                use thiserror::__private::AsDynError as _;
                #[allow(deprecated)]
                match self {
                    #(#arms)*
                }
            }
        })
    } else {
        None
    };

    let provide_method = if input.has_backtrace() {
        let request = quote!(request);
        let arms = input.variants.iter().map(|variant| {
            let ident = &variant.ident;
            match (variant.backtrace_field(), variant.source_field()) {
                (Some(backtrace_field), Some(source_field))
                    if backtrace_field.attrs.backtrace.is_none() =>
                {
                    let backtrace = &backtrace_field.member;
                    let source = &source_field.member;
                    let varsource = quote!(source);
                    let source_provide = if type_is_option(source_field.ty) {
                        quote_spanned! {source.member_span()=>
                            if let ::core::option::Option::Some(source) = #varsource {
                                source.thiserror_provide(#request);
                            }
                        }
                    } else {
                        quote_spanned! {source.member_span()=>
                            #varsource.thiserror_provide(#request);
                        }
                    };
                    let self_provide = if type_is_option(backtrace_field.ty) {
                        quote! {
                            if let ::core::option::Option::Some(backtrace) = backtrace {
                                #request.provide_ref::<std::backtrace::Backtrace>(backtrace);
                            }
                        }
                    } else {
                        quote! {
                            #request.provide_ref::<std::backtrace::Backtrace>(backtrace);
                        }
                    };
                    quote! {
                        #ty::#ident {
                            #backtrace: backtrace,
                            #source: #varsource,
                            ..
                        } => {
                            use thiserror::__private::ThiserrorProvide as _;
                            #source_provide
                            #self_provide
                        }
                    }
                }
                (Some(backtrace_field), Some(source_field))
                    if backtrace_field.member == source_field.member =>
                {
                    let backtrace = &backtrace_field.member;
                    let varsource = quote!(source);
                    let source_provide = if type_is_option(source_field.ty) {
                        quote_spanned! {backtrace.member_span()=>
                            if let ::core::option::Option::Some(source) = #varsource {
                                source.thiserror_provide(#request);
                            }
                        }
                    } else {
                        quote_spanned! {backtrace.member_span()=>
                            #varsource.thiserror_provide(#request);
                        }
                    };
                    quote! {
                        #ty::#ident {#backtrace: #varsource, ..} => {
                            use thiserror::__private::ThiserrorProvide as _;
                            #source_provide
                        }
                    }
                }
                (Some(backtrace_field), _) => {
                    let backtrace = &backtrace_field.member;
                    let body = if type_is_option(backtrace_field.ty) {
                        quote! {
                            if let ::core::option::Option::Some(backtrace) = backtrace {
                                #request.provide_ref::<std::backtrace::Backtrace>(backtrace);
                            }
                        }
                    } else {
                        quote! {
                            #request.provide_ref::<std::backtrace::Backtrace>(backtrace);
                        }
                    };
                    quote! {
                        #ty::#ident {#backtrace: backtrace, ..} => {
                            #body
                        }
                    }
                }
                (None, _) => quote! {
                    #ty::#ident {..} => {}
                },
            }
        });
        Some(quote! {
            fn provide<'_request>(&'_request self, #request: &mut std::error::Request<'_request>) {
                #[allow(deprecated)]
                match self {
                    #(#arms)*
                }
            }
        })
    } else {
        None
    };

    let display_impl = if input.has_display() {
        let mut display_inferred_bounds = InferredBounds::new();
        let has_bonus_display = input.variants.iter().any(|v| {
            v.attrs
                .display
                .as_ref()
                .map_or(false, |display| display.has_bonus_display)
        });
        let use_as_display = use_as_display(has_bonus_display);
        let void_deref = if input.variants.is_empty() {
            Some(quote!(*))
        } else {
            None
        };
        let arms = input.variants.iter().map(|variant| {
            let mut display_implied_bounds = Set::new();
            let display = match &variant.attrs.display {
                Some(display) => {
                    display_implied_bounds.clone_from(&display.implied_bounds);
                    display.to_token_stream()
                }
                None => {
                    let only_field = match &variant.fields[0].member {
                        Member::Named(ident) => ident.clone(),
                        Member::Unnamed(index) => format_ident!("_{}", index),
                    };
                    display_implied_bounds.insert((0, Trait::Display));
                    quote!(::core::fmt::Display::fmt(#only_field, __formatter))
                }
            };
            for (field, bound) in display_implied_bounds {
                let field = &variant.fields[field];
                if field.contains_generic {
                    display_inferred_bounds.insert(field.ty, bound);
                }
            }
            let ident = &variant.ident;
            let pat = fields_pat(&variant.fields);
            quote! {
                #ty::#ident #pat => #display
            }
        });
        let arms = arms.collect::<Vec<_>>();
        let display_where_clause = display_inferred_bounds.augment_where_clause(input.generics);
        Some(quote! {
            #[allow(unused_qualifications)]
            #[automatically_derived]
            impl #impl_generics ::core::fmt::Display for #ty #ty_generics #display_where_clause {
                fn fmt(&self, __formatter: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    #use_as_display
                    #[allow(unused_variables, deprecated, clippy::used_underscore_binding)]
                    match #void_deref self {
                        #(#arms,)*
                    }
                }
            }
        })
    } else {
        None
    };

    let from_impls = input.variants.iter().filter_map(|variant| {
        let from_field = variant.from_field()?;
        let backtrace_field = variant.distinct_backtrace_field();
        let variant = &variant.ident;
        let from = unoptional_type(from_field.ty);
        let body = from_initializer(from_field, backtrace_field);
        Some(quote! {
            #[allow(unused_qualifications)]
            #[automatically_derived]
            impl #impl_generics ::core::convert::From<#from> for #ty #ty_generics #where_clause {
                #[allow(deprecated)]
                fn from(source: #from) -> Self {
                    #ty::#variant #body
                }
            }
        })
    });

    if input.generics.type_params().next().is_some() {
        let self_token = <Token![Self]>::default();
        error_inferred_bounds.insert(self_token, Trait::Debug);
        error_inferred_bounds.insert(self_token, Trait::Display);
    }
    let error_where_clause = error_inferred_bounds.augment_where_clause(input.generics);

    quote! {
        #[allow(unused_qualifications)]
        #[automatically_derived]
        impl #impl_generics std::error::Error for #ty #ty_generics #error_where_clause {
            #source_method
            #provide_method
        }
        #display_impl
        #(#from_impls)*
    }
}

fn fields_pat(fields: &[Field]) -> TokenStream {
    let mut members = fields.iter().map(|field| &field.member).peekable();
    match members.peek() {
        Some(Member::Named(_)) => quote!({ #(#members),* }),
        Some(Member::Unnamed(_)) => {
            let vars = members.map(|member| match member {
                Member::Unnamed(member) => format_ident!("_{}", member),
                Member::Named(_) => unreachable!(),
            });
            quote!((#(#vars),*))
        }
        None => quote!({}),
    }
}

fn use_as_display(needs_as_display: bool) -> Option<TokenStream> {
    if needs_as_display {
        Some(quote! {
            use thiserror::__private::AsDisplay as _;
        })
    } else {
        None
    }
}

fn from_initializer(from_field: &Field, backtrace_field: Option<&Field>) -> TokenStream {
    let from_member = &from_field.member;
    let some_source = if type_is_option(from_field.ty) {
        quote!(::core::option::Option::Some(source))
    } else {
        quote!(source)
    };
    let backtrace = backtrace_field.map(|backtrace_field| {
        let backtrace_member = &backtrace_field.member;
        if type_is_option(backtrace_field.ty) {
            quote! {
                #backtrace_member: ::core::option::Option::Some(std::backtrace::Backtrace::capture()),
            }
        } else {
            quote! {
                #backtrace_member: ::core::convert::From::from(std::backtrace::Backtrace::capture()),
            }
        }
    });
    quote!({
        #from_member: #some_source,
        #backtrace
    })
}

fn type_is_option(ty: &Type) -> bool {
    type_parameter_of_option(ty).is_some()
}

fn unoptional_type(ty: &Type) -> TokenStream {
    let unoptional = type_parameter_of_option(ty).unwrap_or(ty);
    quote!(#unoptional)
}

fn type_parameter_of_option(ty: &Type) -> Option<&Type> {
    let path = match ty {
        Type::Path(ty) => &ty.path,
        _ => return None,
    };

    let last = path.segments.last().unwrap();
    if last.ident != "Option" {
        return None;
    }

    let bracketed = match &last.arguments {
        PathArguments::AngleBracketed(bracketed) => bracketed,
        _ => return None,
    };

    if bracketed.args.len() != 1 {
        return None;
    }

    match &bracketed.args[0] {
        GenericArgument::Type(arg) => Some(arg),
        _ => None,
    }
}
