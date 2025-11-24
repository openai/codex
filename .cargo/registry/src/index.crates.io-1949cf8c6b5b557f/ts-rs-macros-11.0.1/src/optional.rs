use proc_macro2::Span;
use syn::{
    ext::IdentExt, parse::ParseStream, parse_quote, parse_quote_spanned, Error, Expr, Ident, Path,
    Token, Type,
};

use crate::attr::FieldAttr;

/// Indicates whether the field is marked with `#[ts(optional)]`.
/// `#[ts(optional)]` turns an `t: Option<T>` into `t?: T`, while
/// `#[ts(optional = nullable)]` turns it into `t?: T | null`.
#[derive(Default, Clone, Copy)]
pub enum Optional {
    /// Explicitly marked as optional with `#[ts(optional)]`
    Optional { nullable: bool },

    /// Explicitly marked as not optional with `#[ts(optional = false)]`
    NotOptional,

    #[default]
    Inherit,
}

impl Optional {
    pub fn or(self, other: Optional) -> Self {
        match (self, other) {
            (Self::Inherit, other) | (other, Self::Inherit) => other,
            (Self::Optional { nullable: a }, Self::Optional { nullable: b }) => {
                Self::Optional { nullable: a || b }
            }
            _ => other,
        }
    }
}

pub fn parse_optional(input: ParseStream) -> syn::Result<Optional> {
    let optional = if input.peek(Token![=]) {
        input.parse::<Token![=]>()?;
        let span = input.span();

        match Ident::parse_any(input)?.to_string().as_str() {
            "nullable" => Optional::Optional { nullable: true },
            "false" => Optional::NotOptional,
            _ => Err(Error::new(span, "expected 'nullable'"))?,
        }
    } else {
        Optional::Optional { nullable: false }
    };

    Ok(optional)
}

/// Given a field, return a tuple `(is_optional, type)`.  
///
/// `is_optional`:  
/// An expression evaluating to bool, indicating whether the field should be annotated with `?`.
///
/// `type`:  
/// The transformed type of the field after applying the `#[ts(optional)]` annotation.
/// This will be either  
/// - the unmodified type of the field (no optional or `#[ts(optional = nullable)]`) or  
/// - if the field is an `Option<T>`, its inner type `T´ (`#[ts(optional)]`)
pub fn apply(
    crate_rename: &Path,
    for_struct: Optional,
    field_ty: &Type,
    attr: &FieldAttr,
    span: Span,
) -> (Expr, Type) {
    match (for_struct, attr.optional) {
        // explicit `#[ts(optional = false)]` on field, or inherited from struct.
        (Optional::NotOptional, Optional::Inherit) | (_, Optional::NotOptional) => {
            (parse_quote!(false), field_ty.clone())
        }
        // explicit `#[ts(optional)]` on field.
        // It takes precedence over the struct attribute, and is enforced **AT COMPILE TIME**
        (_, Optional::Optional { nullable }) => (
            // expression that evaluates to the string "?", but fails to compile if `ty` is not an `Option`.
            parse_quote_spanned! { span => {
                fn check_that_field_is_option<T: #crate_rename::IsOption>(_: std::marker::PhantomData<T>) {}
                let x: std::marker::PhantomData<#field_ty> = std::marker::PhantomData;
                check_that_field_is_option(x);
                true
            }},
            nullable
                .then(|| field_ty.clone())
                .unwrap_or_else(|| unwrap_option(crate_rename, field_ty)),
        ),
        // Inherited `#[ts(optional)]` from the struct.
        // Acts like `#[ts(optional)]` on a field, but does not error on non-`Option` fields.
        // Instead, it is a no-op.
        (Optional::Optional { nullable }, Optional::Inherit) if attr.type_override.is_none() => (
            parse_quote! {
                <#field_ty as #crate_rename::TS>::IS_OPTION
            },
            nullable
                .then(|| field_ty.clone())
                .unwrap_or_else(|| unwrap_option(crate_rename, field_ty)),
        ),
        // no applicable `#[ts(optional)]` attributes
        _ => {
            // field may be omitted during serialization and has a default value, so the field can be
            // treated as `#[ts(optional = nullable)]`.
            let is_optional = attr.maybe_omitted && attr.has_default;
            (parse_quote!(#is_optional), field_ty.clone())
        }
    }
}

/// Unwraps the given option type, turning `Option<T>` into `T`.
/// otherwise, return the provided type as-is.
fn unwrap_option(crate_rename: &Path, ty: &Type) -> Type {
    parse_quote! {<#ty as #crate_rename::TS>::OptionInnerType}
}
