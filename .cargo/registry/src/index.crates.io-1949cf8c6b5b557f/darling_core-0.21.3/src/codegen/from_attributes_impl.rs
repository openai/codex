use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

use crate::{
    ast::Data,
    codegen::{ExtractAttribute, OuterFromImpl, TraitImpl},
    util::PathList,
};

use super::ForwardAttrs;

pub struct FromAttributesImpl<'a> {
    pub base: TraitImpl<'a>,
    pub attr_names: &'a PathList,
    pub forward_attrs: ForwardAttrs<'a>,
}

impl ToTokens for FromAttributesImpl<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ty_ident = self.base.ident;
        let input = self.param_name();
        let post_transform = self.base.post_transform_call();

        if let Data::Struct(ref data) = self.base.data {
            if data.is_newtype() {
                self.wrap(
                    quote! {
                        fn from_attributes(#input: &[::darling::export::syn::Attribute]) -> ::darling::Result<Self> {
                            ::darling::export::Ok(
                                #ty_ident(::darling::FromAttributes::from_attributes(#input)?)
                            ) #post_transform
                        }
                    },
                    tokens,
                );

                return;
            }
        }

        let passed_attrs = self.forward_attrs.as_initializer();
        let inits = self.base.initializers();
        let default = self.base.fallback_decl();

        let grab_attrs = self.extractor();

        let declare_errors = self.base.declare_errors();
        let require_fields = self.base.require_fields();
        let check_errors = self.base.check_errors();

        self.wrap(
            quote! {
                fn from_attributes(#input: &[::darling::export::syn::Attribute]) -> ::darling::Result<Self> {
                    #declare_errors

                    #grab_attrs

                    #require_fields

                    #check_errors

                    #default

                    ::darling::export::Ok(#ty_ident {
                        #passed_attrs
                        #inits
                    }) #post_transform
                }
            },
            tokens,
        );
    }
}

impl ExtractAttribute for FromAttributesImpl<'_> {
    fn local_declarations(&self) -> TokenStream {
        self.base.local_declarations()
    }

    fn attr_names(&self) -> &PathList {
        self.attr_names
    }

    fn forward_attrs(&self) -> &super::ForwardAttrs<'_> {
        &self.forward_attrs
    }

    fn param_name(&self) -> TokenStream {
        quote!(__di)
    }

    fn attrs_accessor(&self) -> TokenStream {
        self.param_name()
    }

    fn core_loop(&self) -> TokenStream {
        self.base.core_loop()
    }
}

impl<'a> OuterFromImpl<'a> for FromAttributesImpl<'a> {
    fn trait_path(&self) -> syn::Path {
        path!(::darling::FromAttributes)
    }

    fn trait_bound(&self) -> syn::Path {
        path!(::darling::FromMeta)
    }

    fn base(&'a self) -> &'a TraitImpl<'a> {
        &self.base
    }
}
