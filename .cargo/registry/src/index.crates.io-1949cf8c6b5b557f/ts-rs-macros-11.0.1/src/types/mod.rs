use syn::{ext::IdentExt, Expr, Fields, ItemStruct, Result};

use crate::{
    attr::{Attr, StructAttr},
    DerivedTS,
};

mod r#enum;
mod named;
mod newtype;
mod tuple;
mod type_as;
mod type_override;
mod unit;

pub(crate) use r#enum::r#enum_def;

use crate::utils::make_string_literal;

pub(crate) fn struct_def(s: &ItemStruct) -> Result<DerivedTS> {
    let attr = StructAttr::from_attrs(&s.attrs)?;

    let ts_name = attr
        .rename
        .clone()
        .unwrap_or_else(|| make_string_literal(&s.ident.unraw().to_string(), s.ident.span()));
    type_def(&attr, ts_name, &s.fields)
}

fn type_def(attr: &StructAttr, ts_name: Expr, fields: &Fields) -> Result<DerivedTS> {
    attr.assert_validity(fields)?;

    if let Some(attr_type_override) = &attr.type_override {
        return type_override::type_override_struct(attr, ts_name, attr_type_override);
    }
    if let Some(attr_type_as) = &attr.type_as {
        return type_as::type_as_struct(attr, ts_name, attr_type_as);
    }

    match fields {
        Fields::Named(named) => match named.named.len() {
            0 if attr.tag.is_none() => Ok(unit::empty_object(attr, ts_name)),
            _ => named::named(attr, ts_name, named),
        },
        Fields::Unnamed(unnamed) => match unnamed.unnamed.len() {
            0 => Ok(unit::empty_array(attr, ts_name)),
            1 => newtype::newtype(attr, ts_name, unnamed),
            _ => tuple::tuple(attr, ts_name, unnamed),
        },
        Fields::Unit => Ok(unit::null(attr, ts_name)),
    }
}
