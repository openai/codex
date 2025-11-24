use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::Ident;

use crate::{
    ast::Data,
    codegen::{ExtractAttribute, OuterFromImpl, TraitImpl},
    options::{DeriveInputShapeSet, ForwardedField},
    util::PathList,
};

use super::ForwardAttrs;

pub struct FromDeriveInputImpl<'a> {
    pub ident: Option<&'a Ident>,
    pub generics: Option<&'a ForwardedField>,
    pub vis: Option<&'a Ident>,
    pub data: Option<&'a ForwardedField>,
    pub base: TraitImpl<'a>,
    pub attr_names: &'a PathList,
    pub forward_attrs: ForwardAttrs<'a>,
    pub from_ident: bool,
    pub supports: Option<&'a DeriveInputShapeSet>,
}

impl ToTokens for FromDeriveInputImpl<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ty_ident = self.base.ident;
        let input = self.param_name();
        let post_transform = self.base.post_transform_call();

        if let Data::Struct(ref data) = self.base.data {
            if data.is_newtype() {
                self.wrap(
                    quote!{
                        fn from_derive_input(#input: &::darling::export::syn::DeriveInput) -> ::darling::Result<Self> {
                            ::darling::export::Ok(
                                #ty_ident(::darling::FromDeriveInput::from_derive_input(#input)?)
                            ) #post_transform
                        }
                    },
                    tokens,
                );

                return;
            }
        }

        let passed_ident = self
            .ident
            .as_ref()
            .map(|i| quote!(#i: #input.ident.clone(),));
        let passed_vis = self.vis.as_ref().map(|i| quote!(#i: #input.vis.clone(),));
        let passed_attrs = self.forward_attrs.as_initializer();

        let read_generics = self.generics.map(|generics| {
            let ident = &generics.ident;
            let with = generics.with.as_ref().map(|p| quote!(#p)).unwrap_or_else(
                || quote_spanned!(ident.span()=>::darling::FromGenerics::from_generics),
            );
            quote! {
                let #ident = __errors.handle(#with(&#input.generics));
            }
        });

        let pass_generics_to_receiver = self.generics.map(|g| g.as_initializer());

        let check_shape = self
            .supports
            .map(|s| s.validator_path().into_token_stream())
            .unwrap_or_else(|| quote!(::darling::export::Ok));

        let read_data = self
            .data
            .as_ref()
            .map(|i| match &i.with {
                Some(p) => quote!(#p),
                None => quote_spanned!(i.ident.span()=> ::darling::ast::Data::try_from),
            })
            .unwrap_or_else(|| quote!(::darling::export::Ok));

        let supports = self.supports;
        let validate_and_read_data = {
            // If the caller wants `data` read into a field, we can use `data` as the local variable name
            // because we know there are no other fields of that name.
            let let_binding = self.data.map(|d| {
                let ident = &d.ident;
                quote!(let #ident = )
            });
            quote! {
                #supports
                #let_binding __errors.handle(#check_shape(&#input.data).and_then(#read_data));
            }
        };

        let pass_data_to_receiver = self.data.map(|f| f.as_initializer());

        let inits = self.base.initializers();
        let default = if self.from_ident {
            quote!(let __default: Self = ::darling::export::From::from(#input.ident.clone());)
        } else {
            self.base.fallback_decl()
        };

        let grab_attrs = self.extractor();

        let declare_errors = self.base.declare_errors();
        let require_fields = self.base.require_fields();
        let check_errors = self.base.check_errors();

        self.wrap(
            quote! {
                fn from_derive_input(#input: &::darling::export::syn::DeriveInput) -> ::darling::Result<Self> {
                    #declare_errors

                    #grab_attrs

                    #validate_and_read_data

                    #read_generics

                    #require_fields

                    #check_errors

                    #default

                    ::darling::export::Ok(#ty_ident {
                        #passed_ident
                        #pass_generics_to_receiver
                        #passed_vis
                        #passed_attrs
                        #pass_data_to_receiver
                        #inits
                    }) #post_transform
                }
            },
            tokens,
        );
    }
}

impl ExtractAttribute for FromDeriveInputImpl<'_> {
    fn attr_names(&self) -> &PathList {
        self.attr_names
    }

    fn forward_attrs(&self) -> &ForwardAttrs<'_> {
        &self.forward_attrs
    }

    fn param_name(&self) -> TokenStream {
        quote!(__di)
    }

    fn core_loop(&self) -> TokenStream {
        self.base.core_loop()
    }

    fn local_declarations(&self) -> TokenStream {
        self.base.local_declarations()
    }
}

impl<'a> OuterFromImpl<'a> for FromDeriveInputImpl<'a> {
    fn trait_path(&self) -> syn::Path {
        path!(::darling::FromDeriveInput)
    }

    fn trait_bound(&self) -> syn::Path {
        path!(::darling::FromMeta)
    }

    fn base(&'a self) -> &'a TraitImpl<'a> {
        &self.base
    }
}
