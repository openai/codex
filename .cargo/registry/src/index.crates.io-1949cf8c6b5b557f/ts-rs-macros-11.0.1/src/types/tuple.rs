use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Expr, Field, FieldsUnnamed, Path, Result};

use crate::{
    attr::{Attr, ContainerAttr, FieldAttr, StructAttr},
    deps::Dependencies,
    optional::Optional,
    DerivedTS,
};

pub(crate) fn tuple(attr: &StructAttr, ts_name: Expr, fields: &FieldsUnnamed) -> Result<DerivedTS> {
    let crate_rename = attr.crate_rename();
    let mut formatted_fields = Vec::new();
    let mut dependencies = Dependencies::new(crate_rename.clone());
    for field in &fields.unnamed {
        format_field(
            &crate_rename,
            &mut formatted_fields,
            &mut dependencies,
            field,
            attr.optional_fields,
        )?;
    }

    Ok(DerivedTS {
        crate_rename,
        inline: quote! {
            format!(
                "[{}]",
                [#(#formatted_fields),*].join(", ")
            )
        },
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

fn format_field(
    crate_rename: &Path,
    formatted_fields: &mut Vec<TokenStream>,
    dependencies: &mut Dependencies,
    field: &Field,
    struct_optional: Optional,
) -> Result<()> {
    let field_attr = FieldAttr::from_attrs(&field.attrs)?;
    field_attr.assert_validity(field)?;

    if field_attr.skip {
        return Ok(());
    }

    let ty = field_attr.type_as(&field.ty);
    let (is_optional, ty) = crate::optional::apply(
        crate_rename,
        struct_optional,
        &ty,
        &field_attr,
        field.span(),
    );

    let formatted_ty = field_attr
        .type_override
        .map(|t| quote!(#t.to_owned()))
        .unwrap_or_else(|| {
            if field_attr.inline {
                dependencies.append_from(&ty);
                quote!(<#ty as #crate_rename::TS>::inline())
            } else {
                dependencies.push(&ty);
                quote!(<#ty as #crate_rename::TS>::name())
            }
        });

    formatted_fields.push(quote! {
        if #is_optional {
            format!("({})?", #formatted_ty)
        } else {
            #formatted_ty
        }
    });

    Ok(())
}
