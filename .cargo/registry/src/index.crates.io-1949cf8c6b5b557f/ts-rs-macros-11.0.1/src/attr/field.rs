use syn::{
    AngleBracketedGenericArguments, Attribute, Expr, Field, GenericArgument, Ident, PathArguments,
    QSelf, Result, ReturnType, Type, TypeArray, TypeGroup, TypeParen, TypePath, TypePtr,
    TypeReference, TypeSlice, TypeTuple,
};

use super::{parse_assign_from_str, parse_assign_str, Attr, Serde};
use crate::{
    optional::{parse_optional, Optional},
    utils::{extract_docs, parse_attrs},
};

#[derive(Default)]
pub struct FieldAttr {
    type_as: Option<Type>,
    pub type_override: Option<String>,
    pub rename: Option<String>,
    pub inline: bool,
    pub skip: bool,
    pub optional: Optional,
    pub flatten: bool,
    pub docs: Vec<Expr>,

    // serde-specific
    pub using_serde_with: bool,
    // whether the field might be omitted during serialization by skip_serializing{_if}
    pub maybe_omitted: bool,
    pub has_default: bool,
}

impl FieldAttr {
    pub fn from_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut result = parse_attrs::<Self>(attrs)?;

        if cfg!(feature = "serde-compat") && !result.skip {
            let serde_attr = crate::utils::parse_serde_attrs::<FieldAttr>(attrs);
            result = result.merge(serde_attr.0);
        }

        result.docs = extract_docs(attrs);

        Ok(result)
    }

    pub fn type_as(&self, original_type: &Type) -> Type {
        if let Some(mut ty) = self.type_as.clone() {
            replace_underscore(&mut ty, original_type);
            ty
        } else {
            original_type.clone()
        }
    }
}

impl Attr for FieldAttr {
    type Item = Field;

    fn merge(self, other: Self) -> Self {
        Self {
            type_as: self.type_as.or(other.type_as),
            type_override: self.type_override.or(other.type_override),
            rename: self.rename.or(other.rename),
            inline: self.inline || other.inline,
            skip: self.skip || other.skip,
            optional: self.optional.or(other.optional),
            flatten: self.flatten || other.flatten,

            using_serde_with: self.using_serde_with || other.using_serde_with,
            maybe_omitted: self.maybe_omitted || other.maybe_omitted,
            has_default: self.has_default || other.has_default,

            // We can't emit TSDoc for a flattened field
            // and we cant make this invalid in assert_validity because
            // this documentation is totally valid in Rust
            docs: if self.flatten || other.flatten {
                vec![]
            } else {
                [self.docs, other.docs].concat()
            },
        }
    }

    fn assert_validity(&self, field: &Self::Item) -> Result<()> {
        if cfg!(feature = "serde-compat")
            && self.using_serde_with
            && !(self.type_as.is_some() || self.type_override.is_some())
        {
            syn_err_spanned!(
                field;
                r#"using `#[serde(with = "...")]` requires the use of `#[ts(as = "...")]` or `#[ts(type = "...")]`"#
            )
        }

        if self.type_override.is_some() {
            if self.type_as.is_some() {
                syn_err_spanned!(field; "`type` is not compatible with `as`")
            }

            if self.inline {
                syn_err_spanned!(field; "`type` is not compatible with `inline`")
            }

            if self.flatten {
                syn_err_spanned!(
                    field;
                    "`type` is not compatible with `flatten`"
                );
            }
        }

        if self.flatten {
            if self.type_as.is_some() {
                syn_err_spanned!(
                    field;
                    "`as` is not compatible with `flatten`"
                );
            }

            if self.rename.is_some() {
                syn_err_spanned!(
                    field;
                    "`rename` is not compatible with `flatten`"
                );
            }

            if self.inline {
                syn_err_spanned!(
                    field;
                    "`inline` is not compatible with `flatten`"
                );
            }

            if let Optional::Optional { .. } = self.optional {
                syn_err_spanned!(
                    field;
                    "`optional` is not compatible with `flatten`"
                );
            }
        }

        if field.ident.is_none() {
            if self.flatten {
                syn_err_spanned!(
                    field;
                    "`flatten` cannot be used with tuple struct fields"
                );
            }

            if self.rename.is_some() {
                syn_err_spanned!(
                    field;
                    "`rename` cannot be used with tuple struct fields"
                );
            }
        }

        Ok(())
    }
}

impl_parse! {
    FieldAttr(input, out) {
        "as" => out.type_as = Some(parse_assign_from_str(input)?),
        "type" => out.type_override = Some(parse_assign_str(input)?),
        "rename" => out.rename = Some(parse_assign_str(input)?),
        "inline" => out.inline = true,
        "skip" => out.skip = true,
        "optional" => out.optional = parse_optional(input)?,
        "flatten" => out.flatten = true,
    }
}

impl_parse! {
    Serde<FieldAttr>(input, out) {
        "rename" => out.0.rename = Some(parse_assign_str(input)?),
        "skip" => out.0.skip = true,
        "skip_serializing_if" => {
            let _ = parse_assign_str(input)?;
            out.0.maybe_omitted = true;
        },
        "skip_serializing" => {
            out.0.maybe_omitted = true;
        },
        "flatten" => out.0.flatten = true,
        // parse #[serde(default)] to not emit a warning
        "default" => {
            use syn::Token;
            if input.peek(Token![=]) {
                parse_assign_str(input)?;
            }
            out.0.has_default = true;
        },
        "with" => {
            parse_assign_str(input)?;
            out.0.using_serde_with = true;
        },
    }
}

fn replace_underscore(ty: &mut Type, with: &Type) {
    match ty {
        Type::Infer(_) => *ty = with.clone(),
        Type::Array(TypeArray { elem, .. })
        | Type::Group(TypeGroup { elem, .. })
        | Type::Paren(TypeParen { elem, .. })
        | Type::Ptr(TypePtr { elem, .. })
        | Type::Reference(TypeReference { elem, .. })
        | Type::Slice(TypeSlice { elem, .. }) => {
            replace_underscore(elem, with);
        }
        Type::Tuple(TypeTuple { elems, .. }) => {
            for elem in elems {
                replace_underscore(elem, with);
            }
        }
        Type::Path(TypePath { path, qself }) => {
            if let Some(QSelf { ty, .. }) = qself {
                replace_underscore(ty, with);
            }

            for segment in &mut path.segments {
                match &mut segment.arguments {
                    PathArguments::None => (),
                    PathArguments::AngleBracketed(a) => {
                        replace_underscore_in_angle_bracketed(a, with);
                    }
                    PathArguments::Parenthesized(p) => {
                        for input in &mut p.inputs {
                            replace_underscore(input, with);
                        }
                        if let ReturnType::Type(_, output) = &mut p.output {
                            replace_underscore(output, with);
                        }
                    }
                }
            }
        }
        _ => (),
    }
}

fn replace_underscore_in_angle_bracketed(args: &mut AngleBracketedGenericArguments, with: &Type) {
    for arg in &mut args.args {
        match arg {
            GenericArgument::Type(ty) => {
                replace_underscore(ty, with);
            }
            GenericArgument::AssocType(assoc_ty) => {
                replace_underscore(&mut assoc_ty.ty, with);
                if let Some(g) = &mut assoc_ty.generics {
                    replace_underscore_in_angle_bracketed(g, with);
                }
            }
            _ => (),
        }
    }
}
