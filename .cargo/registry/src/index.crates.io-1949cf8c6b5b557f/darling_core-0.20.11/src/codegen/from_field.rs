use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::Ident;

use crate::{
    codegen::{ExtractAttribute, OuterFromImpl, TraitImpl},
    util::PathList,
};

use super::ForwardAttrs;

/// `impl FromField` generator. This is used for parsing an individual
/// field and its attributes.
pub struct FromFieldImpl<'a> {
    pub ident: Option<&'a Ident>,
    pub vis: Option<&'a Ident>,
    pub ty: Option<&'a Ident>,
    pub base: TraitImpl<'a>,
    pub attr_names: &'a PathList,
    pub forward_attrs: ForwardAttrs<'a>,
    pub from_ident: bool,
}

impl ToTokens for FromFieldImpl<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let input = self.param_name();

        let error_declaration = self.base.declare_errors();
        let require_fields = self.base.require_fields();
        let error_check = self.base.check_errors();

        let initializers = self.base.initializers();

        let default = if self.from_ident {
            quote!(let __default: Self = ::darling::export::From::from(#input.ident.clone());)
        } else {
            self.base.fallback_decl()
        };

        let passed_ident = self
            .ident
            .as_ref()
            .map(|i| quote!(#i: #input.ident.clone(),));
        let passed_vis = self.vis.as_ref().map(|i| quote!(#i: #input.vis.clone(),));
        let passed_ty = self.ty.as_ref().map(|i| quote!(#i: #input.ty.clone(),));
        let passed_attrs = self.forward_attrs.as_initializer();

        // Determine which attributes to forward (if any).
        let grab_attrs = self.extractor();
        let post_transform = self.base.post_transform_call();

        self.wrap(
            quote! {
                fn from_field(#input: &::darling::export::syn::Field) -> ::darling::Result<Self> {
                    #error_declaration

                    #grab_attrs

                    #require_fields

                    #error_check

                    #default

                    ::darling::export::Ok(Self {
                        #passed_ident
                        #passed_ty
                        #passed_vis
                        #passed_attrs
                        #initializers
                    }) #post_transform

                }
            },
            tokens,
        );
    }
}

impl ExtractAttribute for FromFieldImpl<'_> {
    fn attr_names(&self) -> &PathList {
        self.attr_names
    }

    fn forward_attrs(&self) -> &super::ForwardAttrs<'_> {
        &self.forward_attrs
    }

    fn param_name(&self) -> TokenStream {
        quote!(__field)
    }

    fn core_loop(&self) -> TokenStream {
        self.base.core_loop()
    }

    fn local_declarations(&self) -> TokenStream {
        self.base.local_declarations()
    }
}

impl<'a> OuterFromImpl<'a> for FromFieldImpl<'a> {
    fn trait_path(&self) -> syn::Path {
        path!(::darling::FromField)
    }

    fn trait_bound(&self) -> syn::Path {
        path!(::darling::FromMeta)
    }

    fn base(&'a self) -> &'a TraitImpl<'a> {
        &self.base
    }
}
