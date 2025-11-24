use quote::quote;
use syn::{Expr, Result, Type};

use crate::{
    attr::{ContainerAttr, EnumAttr, StructAttr},
    deps::Dependencies,
    DerivedTS,
};

pub(crate) fn type_as_struct(
    attr: &StructAttr,
    ts_name: Expr,
    type_as: &Type,
) -> Result<DerivedTS> {
    let crate_rename = attr.crate_rename();

    let mut dependencies = Dependencies::new(crate_rename.clone());
    dependencies.append_from(type_as);

    Ok(DerivedTS {
        crate_rename: crate_rename.clone(),
        inline: quote!(<#type_as as #crate_rename::TS>::inline()),
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

pub(crate) fn type_as_enum(attr: &EnumAttr, ts_name: Expr, type_as: &Type) -> Result<DerivedTS> {
    let crate_rename = attr.crate_rename();

    let mut dependencies = Dependencies::new(crate_rename.clone());
    dependencies.append_from(type_as);

    Ok(DerivedTS {
        crate_rename: crate_rename.clone(),
        inline: quote!(<#type_as as #crate_rename::TS>::inline()),
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
