use crate::ast::{Enum, Field, Input, Struct, Variant};
use crate::attr::Attrs;
use quote::ToTokens;
use std::collections::BTreeSet as Set;
use syn::{Error, GenericArgument, Member, PathArguments, Result, Type};

impl Input<'_> {
    pub(crate) fn validate(&self) -> Result<()> {
        match self {
            Input::Struct(input) => input.validate(),
            Input::Enum(input) => input.validate(),
        }
    }
}

impl Struct<'_> {
    fn validate(&self) -> Result<()> {
        check_non_field_attrs(&self.attrs)?;
        if let Some(transparent) = self.attrs.transparent {
            if self.fields.len() != 1 {
                return Err(Error::new_spanned(
                    transparent.original,
                    "#[error(transparent)] requires exactly one field",
                ));
            }
            if let Some(source) = self.fields.iter().find_map(|f| f.attrs.source) {
                return Err(Error::new_spanned(
                    source,
                    "transparent error struct can't contain #[source]",
                ));
            }
        }
        check_field_attrs(&self.fields)?;
        for field in &self.fields {
            field.validate()?;
        }
        Ok(())
    }
}

impl Enum<'_> {
    fn validate(&self) -> Result<()> {
        check_non_field_attrs(&self.attrs)?;
        let has_display = self.has_display();
        for variant in &self.variants {
            variant.validate()?;
            if has_display && variant.attrs.display.is_none() && variant.attrs.transparent.is_none()
            {
                return Err(Error::new_spanned(
                    variant.original,
                    "missing #[error(\"...\")] display attribute",
                ));
            }
        }
        let mut from_types = Set::new();
        for variant in &self.variants {
            if let Some(from_field) = variant.from_field() {
                let repr = from_field.ty.to_token_stream().to_string();
                if !from_types.insert(repr) {
                    return Err(Error::new_spanned(
                        from_field.original,
                        "cannot derive From because another variant has the same source type",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl Variant<'_> {
    fn validate(&self) -> Result<()> {
        check_non_field_attrs(&self.attrs)?;
        if self.attrs.transparent.is_some() {
            if self.fields.len() != 1 {
                return Err(Error::new_spanned(
                    self.original,
                    "#[error(transparent)] requires exactly one field",
                ));
            }
            if let Some(source) = self.fields.iter().find_map(|f| f.attrs.source) {
                return Err(Error::new_spanned(
                    source,
                    "transparent variant can't contain #[source]",
                ));
            }
        }
        check_field_attrs(&self.fields)?;
        for field in &self.fields {
            field.validate()?;
        }
        Ok(())
    }
}

impl Field<'_> {
    fn validate(&self) -> Result<()> {
        if let Some(display) = &self.attrs.display {
            return Err(Error::new_spanned(
                display.original,
                "not expected here; the #[error(...)] attribute belongs on top of a struct or an enum variant",
            ));
        }
        Ok(())
    }
}

fn check_non_field_attrs(attrs: &Attrs) -> Result<()> {
    if let Some(from) = &attrs.from {
        return Err(Error::new_spanned(
            from,
            "not expected here; the #[from] attribute belongs on a specific field",
        ));
    }
    if let Some(source) = &attrs.source {
        return Err(Error::new_spanned(
            source,
            "not expected here; the #[source] attribute belongs on a specific field",
        ));
    }
    if let Some(backtrace) = &attrs.backtrace {
        return Err(Error::new_spanned(
            backtrace,
            "not expected here; the #[backtrace] attribute belongs on a specific field",
        ));
    }
    if let Some(display) = &attrs.display {
        if attrs.transparent.is_some() {
            return Err(Error::new_spanned(
                display.original,
                "cannot have both #[error(transparent)] and a display attribute",
            ));
        }
    }
    Ok(())
}

fn check_field_attrs(fields: &[Field]) -> Result<()> {
    let mut from_field = None;
    let mut source_field = None;
    let mut backtrace_field = None;
    let mut has_backtrace = false;
    for field in fields {
        if let Some(from) = field.attrs.from {
            if from_field.is_some() {
                return Err(Error::new_spanned(from, "duplicate #[from] attribute"));
            }
            from_field = Some(field);
        }
        if let Some(source) = field.attrs.source {
            if source_field.is_some() {
                return Err(Error::new_spanned(source, "duplicate #[source] attribute"));
            }
            source_field = Some(field);
        }
        if let Some(backtrace) = field.attrs.backtrace {
            if backtrace_field.is_some() {
                return Err(Error::new_spanned(
                    backtrace,
                    "duplicate #[backtrace] attribute",
                ));
            }
            backtrace_field = Some(field);
            has_backtrace = true;
        }
        if let Some(transparent) = field.attrs.transparent {
            return Err(Error::new_spanned(
                transparent.original,
                "#[error(transparent)] needs to go outside the enum or struct, not on an individual field",
            ));
        }
        has_backtrace |= field.is_backtrace();
    }
    if let (Some(from_field), Some(source_field)) = (from_field, source_field) {
        if !same_member(from_field, source_field) {
            return Err(Error::new_spanned(
                from_field.attrs.from,
                "#[from] is only supported on the source field, not any other field",
            ));
        }
    }
    if let Some(from_field) = from_field {
        let max_expected_fields = match backtrace_field {
            Some(backtrace_field) => 1 + !same_member(from_field, backtrace_field) as usize,
            None => 1 + has_backtrace as usize,
        };
        if fields.len() > max_expected_fields {
            return Err(Error::new_spanned(
                from_field.attrs.from,
                "deriving From requires no fields other than source and backtrace",
            ));
        }
    }
    if let Some(source_field) = source_field.or(from_field) {
        if contains_non_static_lifetime(source_field.ty) {
            return Err(Error::new_spanned(
                &source_field.original.ty,
                "non-static lifetimes are not allowed in the source of an error, because std::error::Error requires the source is dyn Error + 'static",
            ));
        }
    }
    Ok(())
}

fn same_member(one: &Field, two: &Field) -> bool {
    match (&one.member, &two.member) {
        (Member::Named(one), Member::Named(two)) => one == two,
        (Member::Unnamed(one), Member::Unnamed(two)) => one.index == two.index,
        _ => unreachable!(),
    }
}

fn contains_non_static_lifetime(ty: &Type) -> bool {
    match ty {
        Type::Path(ty) => {
            let bracketed = match &ty.path.segments.last().unwrap().arguments {
                PathArguments::AngleBracketed(bracketed) => bracketed,
                _ => return false,
            };
            for arg in &bracketed.args {
                match arg {
                    GenericArgument::Type(ty) if contains_non_static_lifetime(ty) => return true,
                    GenericArgument::Lifetime(lifetime) if lifetime.ident != "static" => {
                        return true
                    }
                    _ => {}
                }
            }
            false
        }
        Type::Reference(ty) => ty
            .lifetime
            .as_ref()
            .map_or(false, |lifetime| lifetime.ident != "static"),
        _ => false, // maybe implement later if there are common other cases
    }
}
