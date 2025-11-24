use crate::attr::{self, Attrs};
use crate::generics::ParamsInScope;
use crate::unraw::{IdentUnraw, MemberUnraw};
use proc_macro2::Span;
use std::fmt::{self, Display};
use syn::{
    Data, DataEnum, DataStruct, DeriveInput, Error, Fields, Generics, Ident, Index, Result, Type,
};

pub enum Input<'a> {
    Struct(Struct<'a>),
    Enum(Enum<'a>),
}

pub struct Struct<'a> {
    pub attrs: Attrs<'a>,
    pub ident: Ident,
    pub generics: &'a Generics,
    pub fields: Vec<Field<'a>>,
}

pub struct Enum<'a> {
    pub attrs: Attrs<'a>,
    pub ident: Ident,
    pub generics: &'a Generics,
    pub variants: Vec<Variant<'a>>,
}

pub struct Variant<'a> {
    pub original: &'a syn::Variant,
    pub attrs: Attrs<'a>,
    pub ident: Ident,
    pub fields: Vec<Field<'a>>,
}

pub struct Field<'a> {
    pub original: &'a syn::Field,
    pub attrs: Attrs<'a>,
    pub member: MemberUnraw,
    pub ty: &'a Type,
    pub contains_generic: bool,
}

#[derive(Copy, Clone)]
pub enum ContainerKind {
    Struct,
    TupleStruct,
    UnitStruct,
    StructVariant,
    TupleVariant,
    UnitVariant,
}

impl<'a> Input<'a> {
    pub fn from_syn(node: &'a DeriveInput) -> Result<Self> {
        match &node.data {
            Data::Struct(data) => Struct::from_syn(node, data).map(Input::Struct),
            Data::Enum(data) => Enum::from_syn(node, data).map(Input::Enum),
            Data::Union(_) => Err(Error::new_spanned(
                node,
                "union as errors are not supported",
            )),
        }
    }
}

impl<'a> Struct<'a> {
    fn from_syn(node: &'a DeriveInput, data: &'a DataStruct) -> Result<Self> {
        let mut attrs = attr::get(&node.attrs)?;
        let scope = ParamsInScope::new(&node.generics);
        let fields = Field::multiple_from_syn(&data.fields, &scope)?;
        if let Some(display) = &mut attrs.display {
            let container = ContainerKind::from_struct(data);
            display.expand_shorthand(&fields, container)?;
        }
        Ok(Struct {
            attrs,
            ident: node.ident.clone(),
            generics: &node.generics,
            fields,
        })
    }
}

impl<'a> Enum<'a> {
    fn from_syn(node: &'a DeriveInput, data: &'a DataEnum) -> Result<Self> {
        let attrs = attr::get(&node.attrs)?;
        let scope = ParamsInScope::new(&node.generics);
        let variants = data
            .variants
            .iter()
            .map(|node| {
                let mut variant = Variant::from_syn(node, &scope)?;
                if variant.attrs.display.is_none()
                    && variant.attrs.transparent.is_none()
                    && variant.attrs.fmt.is_none()
                {
                    variant.attrs.display.clone_from(&attrs.display);
                    variant.attrs.transparent = attrs.transparent;
                    variant.attrs.fmt.clone_from(&attrs.fmt);
                }
                if let Some(display) = &mut variant.attrs.display {
                    let container = ContainerKind::from_variant(node);
                    display.expand_shorthand(&variant.fields, container)?;
                }
                Ok(variant)
            })
            .collect::<Result<_>>()?;
        Ok(Enum {
            attrs,
            ident: node.ident.clone(),
            generics: &node.generics,
            variants,
        })
    }
}

impl<'a> Variant<'a> {
    fn from_syn(node: &'a syn::Variant, scope: &ParamsInScope<'a>) -> Result<Self> {
        let attrs = attr::get(&node.attrs)?;
        Ok(Variant {
            original: node,
            attrs,
            ident: node.ident.clone(),
            fields: Field::multiple_from_syn(&node.fields, scope)?,
        })
    }
}

impl<'a> Field<'a> {
    fn multiple_from_syn(fields: &'a Fields, scope: &ParamsInScope<'a>) -> Result<Vec<Self>> {
        fields
            .iter()
            .enumerate()
            .map(|(i, field)| Field::from_syn(i, field, scope))
            .collect()
    }

    fn from_syn(i: usize, node: &'a syn::Field, scope: &ParamsInScope<'a>) -> Result<Self> {
        Ok(Field {
            original: node,
            attrs: attr::get(&node.attrs)?,
            member: match &node.ident {
                Some(name) => MemberUnraw::Named(IdentUnraw::new(name.clone())),
                None => MemberUnraw::Unnamed(Index {
                    index: i as u32,
                    span: Span::call_site(),
                }),
            },
            ty: &node.ty,
            contains_generic: scope.intersects(&node.ty),
        })
    }
}

impl ContainerKind {
    fn from_struct(node: &DataStruct) -> Self {
        match node.fields {
            Fields::Named(_) => ContainerKind::Struct,
            Fields::Unnamed(_) => ContainerKind::TupleStruct,
            Fields::Unit => ContainerKind::UnitStruct,
        }
    }

    fn from_variant(node: &syn::Variant) -> Self {
        match node.fields {
            Fields::Named(_) => ContainerKind::StructVariant,
            Fields::Unnamed(_) => ContainerKind::TupleVariant,
            Fields::Unit => ContainerKind::UnitVariant,
        }
    }
}

impl Display for ContainerKind {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(match self {
            ContainerKind::Struct => "struct",
            ContainerKind::TupleStruct => "tuple struct",
            ContainerKind::UnitStruct => "unit struct",
            ContainerKind::StructVariant => "struct variant",
            ContainerKind::TupleVariant => "tuple variant",
            ContainerKind::UnitVariant => "unit variant",
        })
    }
}
