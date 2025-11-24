/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::ops::Deref;

use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;
use syn::spanned::Spanned;
use syn::Data;
use syn::DataEnum;
use syn::DataStruct;
use syn::DeriveInput;
use syn::Field;
use syn::Fields;

#[derive(Copy, Clone)]
pub(crate) struct FieldsUtil<'a> {
    pub(crate) fields: &'a Fields,
}

impl<'a> Deref for FieldsUtil<'a> {
    type Target = Fields;

    fn deref(&self) -> &Self::Target {
        self.fields
    }
}

impl<'a> FieldsUtil<'a> {
    /// Generate variable names for each field.
    pub(crate) fn gen_field_names(self) -> Vec<syn::Ident> {
        match self.fields {
            Fields::Named(named) => named
                .named
                .iter()
                .map(|f| f.ident.clone().unwrap())
                .collect(),
            Fields::Unnamed(unnamed) => unnamed
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, f)| syn::Ident::new(&format!("f{}", i), f.span()))
                .collect(),
            Fields::Unit => Vec::new(),
        }
    }

    pub(crate) fn gen_fields(self) -> Vec<(syn::Ident, &'a Field)> {
        self.gen_field_names()
            .into_iter()
            .zip(self.fields.iter())
            .collect()
    }
}

#[derive(Copy, Clone)]
pub(crate) struct DataStructUtil<'a> {
    pub(crate) derive_input: &'a DeriveInput,
    pub(crate) data: &'a DataStruct,
}

impl<'a> Deref for DataStructUtil<'a> {
    type Target = DataStruct;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a> DataStructUtil<'a> {
    fn span(self) -> Span {
        self.derive_input.span()
    }

    fn fields(self) -> FieldsUtil<'a> {
        FieldsUtil {
            fields: &self.data.fields,
        }
    }

    /// Return a statement which unpacks the fields into local variables.
    pub(crate) fn unpack_self_fields_expr(
        self,
    ) -> syn::Result<(proc_macro2::TokenStream, Vec<(syn::Ident, &'a Field)>)> {
        let names = self.fields().gen_field_names();
        let name = &self.derive_input.ident;
        let stmt = match &self.data.fields {
            Fields::Named(_fields) => {
                quote_spanned! {
                    self.fields.span() =>
                    let #name { #(#names),* } = self;
                }
            }
            Fields::Unnamed(_fields) => {
                quote_spanned! {
                    self.fields.span() =>
                    let #name(#(#names),*) = self;
                }
            }
            Fields::Unit => quote! {},
        };
        Ok((
            stmt,
            names.into_iter().zip(self.fields().fields.iter()).collect(),
        ))
    }

    pub(crate) fn construct(self, fields: Vec<syn::Expr>) -> syn::Result<syn::Expr> {
        let name = &self.derive_input.ident;
        if fields.len() != self.fields.len() {
            return Err(syn::Error::new(
                self.span(),
                "Number of fields does not match",
            ));
        }
        match &self.fields {
            Fields::Named(_) => {
                let field_names = self.fields().gen_field_names();
                Ok(syn::parse_quote_spanned! {
                    self.span() =>
                    #[allow(clippy::redundant_field_names)]
                    #name { #(#field_names: #fields),* }
                })
            }
            Fields::Unnamed(_) => Ok(syn::parse_quote_spanned! {
                self.span() =>
                #name(#(#fields),*)
            }),
            Fields::Unit => Ok(syn::parse_quote_spanned! {
                self.span() =>
                #name
            }),
        }
    }

    pub(crate) fn match_self(
        self,
        handler: impl Fn(Vec<(syn::Ident, &'a Field)>) -> syn::Result<syn::Expr>,
    ) -> syn::Result<syn::Expr> {
        let (unpack, fields) = self.unpack_self_fields_expr()?;
        let expr = handler(fields)?;
        Ok(syn::parse_quote_spanned! {
            self.span() =>
            {
                #unpack
                #expr
            }
        })
    }
}

#[derive(Copy, Clone)]
pub(crate) struct VariantUtil<'a> {
    pub(crate) variant: &'a syn::Variant,
    pub(crate) enum_util: DataEnumUtil<'a>,
}

impl<'a> Deref for VariantUtil<'a> {
    type Target = syn::Variant;

    fn deref(&self) -> &Self::Target {
        self.variant
    }
}

impl<'a> VariantUtil<'a> {
    fn fields(self) -> FieldsUtil<'a> {
        FieldsUtil {
            fields: &self.variant.fields,
        }
    }

    pub(crate) fn match_arm(
        self,
        expr: impl FnOnce(Vec<(syn::Ident, &'a Field)>) -> syn::Result<syn::Expr>,
    ) -> syn::Result<syn::Arm> {
        let expr = expr(self.fields().gen_fields())?;

        let name = &self.enum_util.derive_input.ident;
        let variant_name = &self.variant.ident;
        let field_names = self.fields().gen_field_names();
        match &self.variant.fields {
            Fields::Named(_) => Ok(syn::parse_quote_spanned! {
                self.variant.span() =>
                #name::#variant_name { #(#field_names),* } => { #expr }
            }),
            Fields::Unnamed(_) => Ok(syn::parse_quote_spanned! {
                self.variant.span() =>
                #name::#variant_name(#(#field_names),*) => { #expr }
            }),
            Fields::Unit => Ok(syn::parse_quote_spanned! {
                self.variant.span() =>
                #name::#variant_name => { #expr }
            }),
        }
    }

    pub(crate) fn construct(self, fields: Vec<syn::Expr>) -> syn::Result<syn::Expr> {
        let name = &self.enum_util.derive_input.ident;
        let variant_name = &self.variant.ident;
        let variant = match &self.variant.fields {
            Fields::Named(_) => {
                let field_names = self.fields().gen_field_names();
                syn::parse_quote_spanned! {
                    self.variant.span() =>
                    #[allow(clippy::redundant_field_names)]
                    #name::#variant_name { #(#field_names: #fields),* }
                }
            }
            Fields::Unnamed(_) => {
                syn::parse_quote_spanned! {
                    self.variant.span() =>
                    #name::#variant_name(#(#fields),*)
                }
            }
            Fields::Unit => syn::parse_quote_spanned! {
                self.variant.span() =>
                #name::#variant_name
            },
        };
        Ok(variant)
    }
}

#[derive(Copy, Clone)]
pub(crate) struct DataEnumUtil<'a> {
    pub(crate) derive_input: &'a DeriveInput,
    pub(crate) data: &'a DataEnum,
}

impl<'a> Deref for DataEnumUtil<'a> {
    type Target = DataEnum;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a> DataEnumUtil<'a> {
    pub(crate) fn span(self) -> Span {
        self.derive_input.span()
    }

    pub(crate) fn match_self(
        self,
        variant: impl Fn(VariantUtil<'a>, Vec<(syn::Ident, &'a Field)>) -> syn::Result<syn::Expr>,
    ) -> syn::Result<syn::Expr> {
        let arms = self
            .variants()
            .map(|v| v.match_arm(|fields| variant(v, fields)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(syn::parse_quote_spanned! {
            self.data.enum_token.span() =>
            match self {
                #(#arms),*
            }
        })
    }
}

impl<'a> DataEnumUtil<'a> {
    pub(crate) fn variants(self) -> impl Iterator<Item = VariantUtil<'a>> {
        self.data.variants.iter().map(move |variant| VariantUtil {
            variant,
            enum_util: self,
        })
    }
}

#[derive(Copy, Clone)]
pub(crate) enum StructOrEnumVariant<'a> {
    Struct(DataStructUtil<'a>),
    EnumVariant(VariantUtil<'a>),
}

impl<'a> StructOrEnumVariant<'a> {
    pub(crate) fn span(self) -> Span {
        match self {
            StructOrEnumVariant::Struct(data) => data.span(),
            StructOrEnumVariant::EnumVariant(variant) => variant.span(),
        }
    }

    pub(crate) fn construct(self, fields: Vec<syn::Expr>) -> syn::Result<syn::Expr> {
        match self {
            StructOrEnumVariant::Struct(data) => data.construct(fields),
            StructOrEnumVariant::EnumVariant(variant) => variant.construct(fields),
        }
    }
}

#[derive(Copy, Clone)]
pub(crate) enum DeriveInputUtil<'a> {
    Struct(DataStructUtil<'a>),
    Enum(DataEnumUtil<'a>),
}

impl<'a> Deref for DeriveInputUtil<'a> {
    type Target = DeriveInput;

    fn deref(&self) -> &Self::Target {
        match self {
            DeriveInputUtil::Struct(data) => data.derive_input,
            DeriveInputUtil::Enum(data) => data.derive_input,
        }
    }
}

impl<'a> DeriveInputUtil<'a> {
    pub(crate) fn new(derive_input: &'a DeriveInput) -> syn::Result<Self> {
        match &derive_input.data {
            Data::Struct(data) => Ok(DeriveInputUtil::Struct(DataStructUtil {
                derive_input,
                data,
            })),
            Data::Enum(data) => Ok(DeriveInputUtil::Enum(DataEnumUtil { derive_input, data })),
            Data::Union(_) => Err(syn::Error::new_spanned(
                derive_input,
                "Only structs and enums are supported",
            )),
        }
    }

    pub(crate) fn span(self) -> Span {
        match self {
            DeriveInputUtil::Struct(data) => data.span(),
            DeriveInputUtil::Enum(data) => data.span(),
        }
    }

    /// Generate expressions with callback for a struct or for each enum variant.
    pub(crate) fn match_self(
        self,
        handler: impl Fn(StructOrEnumVariant, Vec<(syn::Ident, &'a Field)>) -> syn::Result<syn::Expr>,
    ) -> syn::Result<syn::Expr> {
        match self {
            DeriveInputUtil::Struct(data) => {
                data.match_self(|fields| handler(StructOrEnumVariant::Struct(data), fields))
            }
            DeriveInputUtil::Enum(data) => data.match_self(|variant, fields| {
                handler(StructOrEnumVariant::EnumVariant(variant), fields)
            }),
        }
    }

    /// Process each field of struct of enum with provided callback.
    pub(crate) fn for_each_field(
        self,
        field_handler: impl Fn(&syn::Ident, &Field) -> syn::Result<TokenStream>,
    ) -> syn::Result<syn::Expr> {
        self.match_self(|struct_or_enum_variant, fields| {
            let handlers: Vec<TokenStream> = fields
                .iter()
                .map(|(ident, f)| field_handler(ident, f))
                .collect::<syn::Result<_>>()?;
            Ok(syn::parse_quote_spanned! {
                struct_or_enum_variant.span() =>
                {
                    #(#handlers)*
                }
            })
        })
    }

    pub(crate) fn generics(self) -> GenericsUtil<'a> {
        match self {
            DeriveInputUtil::Struct(data) => GenericsUtil::new(&data.derive_input.generics),
            DeriveInputUtil::Enum(data) => GenericsUtil::new(&data.derive_input.generics),
        }
    }
}

#[derive(Copy, Clone)]
pub(crate) struct GenericsUtil<'a> {
    pub(crate) generics: &'a syn::Generics,
}

impl<'a> GenericsUtil<'a> {
    pub(crate) fn new(generics: &'a syn::Generics) -> Self {
        GenericsUtil { generics }
    }

    pub(crate) fn assert_only_lifetime_params(self) -> syn::Result<Vec<&'a syn::LifetimeParam>> {
        let mut lifetimes = Vec::new();
        for param in &self.generics.params {
            match param {
                syn::GenericParam::Lifetime(param) => lifetimes.push(param),
                _ => {
                    return Err(syn::Error::new_spanned(
                        param,
                        "only lifetime parameters are supported (no type or const parameters)",
                    ));
                }
            }
        }
        Ok(lifetimes)
    }

    pub(crate) fn assert_only_type_params(self) -> syn::Result<Vec<&'a syn::TypeParam>> {
        let mut type_params = Vec::new();
        for param in &self.generics.params {
            match param {
                syn::GenericParam::Type(param) => type_params.push(param),
                _ => {
                    return Err(syn::Error::new_spanned(
                        param,
                        "only type parameters are supported (no lifetime or const parameters)",
                    ));
                }
            }
        }
        Ok(type_params)
    }

    pub(crate) fn assert_at_most_one_lifetime_param(
        self,
    ) -> syn::Result<Option<&'a syn::LifetimeParam>> {
        let mut lifetime_params = self.generics.lifetimes();
        let Some(lt) = lifetime_params.next() else {
            return Ok(None);
        };
        if lifetime_params.next().is_some() {
            return Err(syn::Error::new_spanned(
                lt,
                "expecting at most one lifetime parameter",
            ));
        }
        Ok(Some(lt))
    }
}

impl<'a> Deref for GenericsUtil<'a> {
    type Target = syn::Generics;

    fn deref(&self) -> &Self::Target {
        self.generics
    }
}
