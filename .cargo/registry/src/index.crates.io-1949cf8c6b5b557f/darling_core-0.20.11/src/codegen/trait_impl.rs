use proc_macro2::TokenStream;
use quote::quote;
use syn::{Generics, Ident};

use crate::ast::{Data, Fields};
use crate::codegen::{
    error::{ErrorCheck, ErrorDeclaration},
    DefaultExpression, Field, FieldsGen, PostfixTransform, Variant,
};
use crate::usage::{CollectTypeParams, IdentSet, Purpose};

#[derive(Debug)]
pub struct TraitImpl<'a> {
    pub ident: &'a Ident,
    pub generics: &'a Generics,
    pub data: Data<Variant<'a>, Field<'a>>,
    pub default: Option<DefaultExpression<'a>>,
    pub post_transform: Option<&'a PostfixTransform>,
    pub allow_unknown_fields: bool,
}

impl<'a> TraitImpl<'a> {
    /// Get all declared type parameters.
    pub fn declared_type_params(&self) -> IdentSet {
        self.generics
            .type_params()
            .map(|tp| tp.ident.clone())
            .collect()
    }

    /// Get the type parameters which are used by non-skipped, non-magic fields.
    /// These type parameters will have a `FromMeta` bound applied to them in emitted
    /// code.
    pub fn used_type_params(&self) -> IdentSet {
        self.type_params_matching(|f| !f.skip, |v| !v.skip)
    }

    fn type_params_matching<F, V>(&self, field_filter: F, variant_filter: V) -> IdentSet
    where
        F: Fn(&&Field<'_>) -> bool,
        V: Fn(&&Variant<'_>) -> bool,
    {
        let declared = self.declared_type_params();
        match self.data {
            Data::Struct(ref v) => self.type_params_in_fields(v, &field_filter, &declared),
            Data::Enum(ref v) => {
                v.iter()
                    .filter(variant_filter)
                    .fold(Default::default(), |mut state, variant| {
                        state.extend(self.type_params_in_fields(
                            &variant.data,
                            &field_filter,
                            &declared,
                        ));
                        state
                    })
            }
        }
    }

    /// Get the type parameters of all fields in a set matching some filter
    fn type_params_in_fields<'b, F>(
        &'b self,
        fields: &'b Fields<Field<'a>>,
        field_filter: F,
        declared: &IdentSet,
    ) -> IdentSet
    where
        F: Fn(&&'b Field<'_>) -> bool,
    {
        fields
            .iter()
            .filter(field_filter)
            .collect_type_params_cloned(&Purpose::BoundImpl.into(), declared)
    }
}

impl<'a> TraitImpl<'a> {
    /// Gets the `let` declaration for errors accumulated during parsing.
    pub fn declare_errors(&self) -> ErrorDeclaration {
        ErrorDeclaration::default()
    }

    /// Gets the check which performs an early return if errors occurred during parsing.
    pub fn check_errors(&self) -> ErrorCheck<'_> {
        ErrorCheck::default()
    }

    /// Generate local variable declarations for all fields.
    pub(in crate::codegen) fn local_declarations(&self) -> TokenStream {
        if let Data::Struct(ref vd) = self.data {
            let vdr = vd.as_ref().map(Field::as_declaration);
            let decls = vdr.fields.as_slice();
            quote!(#(#decls)*)
        } else {
            quote!()
        }
    }

    pub(in crate::codegen) fn post_transform_call(&self) -> Option<TokenStream> {
        self.post_transform.map(|pt| quote!(#pt))
    }

    /// Generate local variable declaration and initialization for instance from which missing fields will be taken.
    pub(in crate::codegen) fn fallback_decl(&self) -> TokenStream {
        let default = self.default.as_ref().map(DefaultExpression::as_declaration);
        quote!(#default)
    }

    pub fn require_fields(&self) -> TokenStream {
        if let Data::Struct(ref vd) = self.data {
            let check_nones = vd.as_ref().map(Field::as_presence_check);
            let checks = check_nones.fields.as_slice();

            // If a field was marked `flatten`, now is the time to process any unclaimed meta items
            // and mark the field as having been seen.
            let flatten_field_init = vd.fields.iter().find(|f| f.flatten).map(|v| {
                v.as_flatten_initializer(vd.fields.iter().filter_map(Field::as_name).collect())
            });

            quote! {
                #flatten_field_init
                #(#checks)*
            }
        } else {
            quote!()
        }
    }

    pub(in crate::codegen) fn initializers(&self) -> TokenStream {
        self.make_field_ctx().initializers()
    }

    /// Generate the loop which walks meta items looking for property matches.
    pub(in crate::codegen) fn core_loop(&self) -> TokenStream {
        self.make_field_ctx().core_loop()
    }

    fn make_field_ctx(&'a self) -> FieldsGen<'a> {
        match self.data {
            Data::Enum(_) => panic!("Core loop on enums isn't supported"),
            Data::Struct(ref data) => FieldsGen::new(data, self.allow_unknown_fields),
        }
    }
}
