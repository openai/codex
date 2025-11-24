mod from_serde;

use crate::attr::{ContainerAttrs, FieldAttrs, VariantAttrs};
use crate::idents::{GENERATOR, SCHEMA};
use from_serde::FromSerde;
use proc_macro2::TokenStream;
use serde_derive_internals::ast as serde_ast;
use serde_derive_internals::{Ctxt, Derive};
use std::collections::BTreeSet;

pub struct Container<'a> {
    pub ident: syn::Ident,
    pub serde_attrs: serde_derive_internals::attr::Container,
    pub data: Data<'a>,
    pub generics: syn::Generics,
    pub attrs: ContainerAttrs,
    /// A set of type params that are used in a `rename` attribute format string, e.g. `T` and `U`
    /// in `#[schemars(rename = "StructFor{T}And{U}")]`. This does not include const params.
    pub rename_type_params: BTreeSet<&'a syn::Ident>,
    /// A set of type params that are "relevant" to the impl, i.e. excluding params only used in
    /// `PhantomData` or skipped fields
    pub relevant_type_params: BTreeSet<&'a syn::Ident>,
}

pub enum Data<'a> {
    Enum(Vec<Variant<'a>>),
    Struct(serde_ast::Style, Vec<Field<'a>>),
}

pub struct Variant<'a> {
    pub ident: syn::Ident,
    pub serde_attrs: serde_derive_internals::attr::Variant,
    pub style: serde_ast::Style,
    pub fields: Vec<Field<'a>>,
    pub original: &'a syn::Variant,
    pub attrs: VariantAttrs,
}

pub struct Field<'a> {
    pub member: syn::Member,
    pub serde_attrs: serde_derive_internals::attr::Field,
    pub ty: &'a syn::Type,
    pub original: &'a syn::Field,
    pub attrs: FieldAttrs,
}

impl<'a> Container<'a> {
    pub fn from_ast(item: &'a syn::DeriveInput) -> syn::Result<Container<'a>> {
        let ctxt = Ctxt::new();
        let result = serde_ast::Container::from_ast(&ctxt, item, Derive::Deserialize)
            .ok_or(())
            .map(|serde| Self::from_serde(&ctxt, serde));

        ctxt.check()
            .map(|()| result.expect("from_ast set no errors on Ctxt, so should have returned Ok"))
    }

    pub fn transparent_field(&'a self) -> Option<&'a Field<'a>> {
        if self.serde_attrs.transparent() {
            if let Data::Struct(_, fields) = &self.data {
                return fields.iter().find(|f| f.serde_attrs.transparent());
            }
        }

        None
    }

    pub fn add_mutators(&self, mutators: &mut Vec<TokenStream>) {
        self.attrs.common.add_mutators(mutators);
    }

    pub fn name(&'a self) -> std::borrow::Cow<'a, str> {
        if self.attrs.rename_format_string.is_none() {
            if let Some(remote_name) = self.serde_attrs.remote().and_then(|r| r.segments.last()) {
                return remote_name.ident.to_string().into();
            }
        }

        self.serde_attrs.name().deserialize_name().into()
    }
}

impl Variant<'_> {
    pub fn name(&self) -> Name {
        Name(self.serde_attrs.name())
    }

    pub fn is_unit(&self) -> bool {
        matches!(self.style, serde_ast::Style::Unit)
    }

    pub fn add_mutators(&self, mutators: &mut Vec<TokenStream>) {
        self.attrs.common.add_mutators(mutators);
    }

    pub fn with_contract_check(&self, action: TokenStream) -> TokenStream {
        with_contract_check(
            self.serde_attrs.skip_deserializing(),
            self.serde_attrs.skip_serializing(),
            action,
        )
    }
}

impl Field<'_> {
    pub fn name(&self) -> Name {
        Name(self.serde_attrs.name())
    }

    pub fn add_mutators(&self, mutators: &mut Vec<TokenStream>) {
        self.attrs.common.add_mutators(mutators);
        self.attrs.validation.add_mutators(mutators);

        if self.serde_attrs.skip_deserializing() {
            mutators.push(quote! {
                #SCHEMA.insert("readOnly".into(), true.into());
            });
        }
        if self.serde_attrs.skip_serializing() {
            mutators.push(quote! {
                #SCHEMA.insert("writeOnly".into(), true.into());
            });
        }
    }

    pub fn with_contract_check(&self, action: TokenStream) -> TokenStream {
        with_contract_check(
            self.serde_attrs.skip_deserializing(),
            self.serde_attrs.skip_serializing(),
            action,
        )
    }
}

pub struct Name<'a>(&'a serde_derive_internals::attr::Name);

impl quote::ToTokens for Name<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ser_name = self.0.serialize_name();
        let de_name = self.0.deserialize_name();
        if ser_name == de_name {
            ser_name.to_tokens(tokens);
        } else {
            quote! {
                if #GENERATOR.contract().is_serialize() {
                    #ser_name
                } else {
                    #de_name
                }
            }
            .to_tokens(tokens);
        }
    }
}

fn with_contract_check(
    skip_deserializing: bool,
    skip_serializing: bool,
    action: TokenStream,
) -> TokenStream {
    match (skip_deserializing, skip_serializing) {
        (true, true) => TokenStream::new(),
        (true, false) => quote! {
            if #GENERATOR.contract().is_serialize() {
                #action
            }
        },
        (false, true) => quote! {
            if #GENERATOR.contract().is_deserialize() {
                #action
            }
        },
        (false, false) => action,
    }
}
