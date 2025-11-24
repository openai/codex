use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::Ident;

use crate::codegen::{ExtractAttribute, ForwardAttrs, OuterFromImpl, TraitImpl};
use crate::options::DataShape;
use crate::util::PathList;

pub struct FromVariantImpl<'a> {
    pub base: TraitImpl<'a>,
    /// If set, the ident of the field into which the variant ident should be placed.
    ///
    /// This is one of `darling`'s "magic fields", which allow a type deriving a `darling`
    /// trait to get fields from the input `syn` element added to the deriving struct
    /// automatically.
    pub ident: Option<&'a Ident>,
    /// If set, the ident of the field into which the transformed output of the input
    /// variant's fields should be placed.
    ///
    /// This is one of `darling`'s "magic fields".
    pub fields: Option<&'a Ident>,
    /// If set, the ident of the field into which the discriminant of the input variant
    /// should be placed. The receiving field must be an `Option` as not all enums have
    /// discriminants.
    ///
    /// This is one of `darling`'s "magic fields".
    pub discriminant: Option<&'a Ident>,
    pub attr_names: &'a PathList,
    pub forward_attrs: ForwardAttrs<'a>,
    pub from_ident: bool,
    pub supports: Option<&'a DataShape>,
}

impl ToTokens for FromVariantImpl<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let input = self.param_name();
        let extractor = self.extractor();
        let passed_ident = self
            .ident
            .as_ref()
            .map(|i| quote!(#i: #input.ident.clone(),));
        let passed_discriminant = self
            .discriminant
            .as_ref()
            .map(|i| quote!(#i: #input.discriminant.as_ref().map(|(_, expr)| expr.clone()),));
        let passed_attrs = self.forward_attrs.as_initializer();
        let passed_fields = self
            .fields
            .as_ref()
            .map(|i| quote!(#i: ::darling::ast::Fields::try_from(&#input.fields)?,));

        let inits = self.base.initializers();
        let post_transform = self.base.post_transform_call();

        let default = if self.from_ident {
            quote!(let __default: Self = ::darling::export::From::from(#input.ident.clone());)
        } else {
            self.base.fallback_decl()
        };

        let supports = self.supports.map(|i| {
            quote! {
                __errors.handle(#i.check(&#input.fields));
            }
        });

        let error_declaration = self.base.declare_errors();
        let require_fields = self.base.require_fields();
        let error_check = self.base.check_errors();

        self.wrap(
            quote!(
                fn from_variant(#input: &::darling::export::syn::Variant) -> ::darling::Result<Self> {
                    #error_declaration

                    #extractor

                    #supports

                    #require_fields

                    #error_check

                    #default

                    ::darling::export::Ok(Self {
                        #passed_ident
                        #passed_discriminant
                        #passed_attrs
                        #passed_fields
                        #inits
                    }) #post_transform
                }
            ),
            tokens,
        );
    }
}

impl ExtractAttribute for FromVariantImpl<'_> {
    fn local_declarations(&self) -> TokenStream {
        self.base.local_declarations()
    }

    fn attr_names(&self) -> &PathList {
        self.attr_names
    }

    fn forward_attrs(&self) -> &ForwardAttrs<'_> {
        &self.forward_attrs
    }

    fn param_name(&self) -> TokenStream {
        quote!(__variant)
    }

    fn core_loop(&self) -> TokenStream {
        self.base.core_loop()
    }
}

impl<'a> OuterFromImpl<'a> for FromVariantImpl<'a> {
    fn trait_path(&self) -> syn::Path {
        path!(::darling::FromVariant)
    }

    fn trait_bound(&self) -> syn::Path {
        path!(::darling::FromMeta)
    }

    fn base(&'a self) -> &'a TraitImpl<'a> {
        &self.base
    }
}
