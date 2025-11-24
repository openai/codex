use quote::quote;
use syn::{Expr, FieldsUnnamed, Result};

use crate::{
    attr::{Attr, ContainerAttr, FieldAttr, StructAttr},
    deps::Dependencies,
    DerivedTS,
};

pub(crate) fn newtype(
    attr: &StructAttr,
    ts_name: Expr,
    fields: &FieldsUnnamed,
) -> Result<DerivedTS> {
    let inner = fields.unnamed.first().unwrap();

    let field_attr = FieldAttr::from_attrs(&inner.attrs)?;
    field_attr.assert_validity(inner)?;

    let crate_rename = attr.crate_rename();

    if field_attr.skip {
        return Ok(super::unit::null(attr, ts_name));
    }

    let inner_ty = field_attr.type_as(&inner.ty);

    let mut dependencies = Dependencies::new(crate_rename.clone());

    match (&field_attr.type_override, field_attr.inline) {
        (Some(_), _) => (),
        (None, true) => dependencies.append_from(&inner_ty),
        (None, false) => dependencies.push(&inner_ty),
    };

    let inline_def = match field_attr.type_override {
        Some(ref o) => quote!(#o.to_owned()),
        None if field_attr.inline => quote!(<#inner_ty as #crate_rename::TS>::inline()),
        None => quote!(<#inner_ty as #crate_rename::TS>::name()),
    };

    Ok(DerivedTS {
        crate_rename,
        inline: inline_def,
        inline_flattened: None,
        docs: attr.docs.clone(),
        dependencies,
        export: attr.export,
        export_to: attr.export_to.clone(),
        ts_name,
        concrete: attr.concrete.clone(),
        bound: attr.bound.clone(),
    })
}
