use super::*;
use serde_derive_internals::ast as serde_ast;
use serde_derive_internals::Ctxt;

pub trait FromSerde: Sized {
    type SerdeType;

    fn from_serde(errors: &Ctxt, serde: Self::SerdeType) -> Self;

    fn vec_from_serde(errors: &Ctxt, serdes: Vec<Self::SerdeType>) -> Vec<Self> {
        serdes
            .into_iter()
            .map(|s| Self::from_serde(errors, s))
            .collect()
    }
}

impl<'a> FromSerde for Container<'a> {
    type SerdeType = serde_ast::Container<'a>;

    fn from_serde(errors: &Ctxt, serde: Self::SerdeType) -> Self {
        let data = Data::from_serde(errors, serde.data);
        let attrs = ContainerAttrs::new(&serde.original.attrs, &data, errors);
        let rename_type_params = match &attrs.rename_format_string {
            Some(s) => crate::name::get_rename_format_type_params(errors, s, serde.generics),
            None => BTreeSet::new(),
        };

        let mut cont = Self {
            ident: serde.ident,
            serde_attrs: serde.attrs,
            data,
            attrs,
            rename_type_params,
            // `relevant_type_params` populated in find_trait_bounds()
            relevant_type_params: BTreeSet::new(),
            // `generics.where_clause` extended in find_trait_bounds()
            generics: serde.generics.clone(),
        };

        crate::bound::find_trait_bounds(serde.generics, &mut cont);

        cont
    }
}

impl<'a> FromSerde for Data<'a> {
    type SerdeType = serde_ast::Data<'a>;

    fn from_serde(errors: &Ctxt, serde: Self::SerdeType) -> Self {
        match serde {
            serde_ast::Data::Enum(variants) => {
                Data::Enum(Variant::vec_from_serde(errors, variants))
            }
            serde_ast::Data::Struct(style, fields) => {
                Data::Struct(style, Field::vec_from_serde(errors, fields))
            }
        }
    }
}

impl<'a> FromSerde for Variant<'a> {
    type SerdeType = serde_ast::Variant<'a>;

    fn from_serde(errors: &Ctxt, serde: Self::SerdeType) -> Self {
        Self {
            ident: serde.ident,
            serde_attrs: serde.attrs,
            style: serde.style,
            fields: Field::vec_from_serde(errors, serde.fields),
            original: serde.original,
            attrs: VariantAttrs::new(&serde.original.attrs, errors),
        }
    }
}

impl<'a> FromSerde for Field<'a> {
    type SerdeType = serde_ast::Field<'a>;

    fn from_serde(errors: &Ctxt, serde: Self::SerdeType) -> Self {
        Self {
            member: serde.member,
            serde_attrs: serde.attrs,
            ty: serde.ty,
            original: serde.original,
            attrs: FieldAttrs::new(&serde.original.attrs, errors),
        }
    }
}
