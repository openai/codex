use super::*;
use crate::attr::Attrs;
use serde_derive_internals::ast as serde_ast;
use serde_derive_internals::Ctxt;

pub trait FromSerde: Sized {
    type SerdeType;

    fn from_serde(errors: &Ctxt, serde: Self::SerdeType) -> Result<Self, ()>;

    fn vec_from_serde(errors: &Ctxt, serdes: Vec<Self::SerdeType>) -> Result<Vec<Self>, ()> {
        let mut result = Vec::with_capacity(serdes.len());
        for s in serdes {
            result.push(Self::from_serde(errors, s)?)
        }
        Ok(result)
    }
}

impl<'a> FromSerde for Container<'a> {
    type SerdeType = serde_ast::Container<'a>;

    fn from_serde(errors: &Ctxt, serde: Self::SerdeType) -> Result<Self, ()> {
        Ok(Self {
            ident: serde.ident,
            serde_attrs: serde.attrs,
            data: Data::from_serde(errors, serde.data)?,
            generics: serde.generics.clone(),
            original: serde.original,
            // FIXME this allows with/schema_with attribute on containers
            attrs: Attrs::new(&serde.original.attrs, errors),
        })
    }
}

impl<'a> FromSerde for Data<'a> {
    type SerdeType = serde_ast::Data<'a>;

    fn from_serde(errors: &Ctxt, serde: Self::SerdeType) -> Result<Self, ()> {
        Ok(match serde {
            serde_ast::Data::Enum(variants) => {
                Data::Enum(Variant::vec_from_serde(errors, variants)?)
            }
            serde_ast::Data::Struct(style, fields) => {
                Data::Struct(style, Field::vec_from_serde(errors, fields)?)
            }
        })
    }
}

impl<'a> FromSerde for Variant<'a> {
    type SerdeType = serde_ast::Variant<'a>;

    fn from_serde(errors: &Ctxt, serde: Self::SerdeType) -> Result<Self, ()> {
        Ok(Self {
            ident: serde.ident,
            serde_attrs: serde.attrs,
            style: serde.style,
            fields: Field::vec_from_serde(errors, serde.fields)?,
            original: serde.original,
            attrs: Attrs::new(&serde.original.attrs, errors),
        })
    }
}

impl<'a> FromSerde for Field<'a> {
    type SerdeType = serde_ast::Field<'a>;

    fn from_serde(errors: &Ctxt, serde: Self::SerdeType) -> Result<Self, ()> {
        Ok(Self {
            member: serde.member,
            serde_attrs: serde.attrs,
            ty: serde.ty,
            original: serde.original,
            attrs: Attrs::new(&serde.original.attrs, errors),
            validation_attrs: ValidationAttrs::new(&serde.original.attrs, errors),
        })
    }
}
