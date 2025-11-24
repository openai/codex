use ident_case::RenameRule;

use crate::ast::{Data, Fields, Style};
use crate::codegen;
use crate::codegen::PostfixTransform;
use crate::error::Accumulator;
use crate::options::{DefaultExpression, InputField, InputVariant, ParseAttribute, ParseData};
use crate::{Error, FromMeta, Result};

/// A struct or enum which should have `FromMeta` or `FromDeriveInput` implementations
/// generated.
#[derive(Debug, Clone)]
pub struct Core {
    /// The type identifier.
    pub ident: syn::Ident,

    /// The type's generics. If the type does not use any generics, this will
    /// be an empty instance.
    pub generics: syn::Generics,

    /// Controls whether missing properties should cause errors or should be filled by
    /// the result of a function call. This can be overridden at the field level.
    pub default: Option<DefaultExpression>,

    /// The rule that should be used to rename all fields/variants in the container.
    pub rename_rule: RenameRule,

    /// A transform which will be called on `darling::Result<Self>`. It must either be
    /// an `FnOnce(T) -> T` when `map` is used, or `FnOnce(T) -> darling::Result<T>` when
    /// `and_then` is used.
    ///
    /// `map` and `and_then` are mutually-exclusive to avoid confusion about the order in
    /// which the two are applied.
    pub post_transform: Option<codegen::PostfixTransform>,

    /// The body of the _deriving_ type.
    pub data: Data<InputVariant, InputField>,

    /// The custom bound to apply to the generated impl
    pub bound: Option<Vec<syn::WherePredicate>>,

    /// Whether or not unknown fields should produce an error at compilation time.
    pub allow_unknown_fields: Option<bool>,
}

impl Core {
    /// Partially initializes `Core` by reading the identity, generics, and body shape.
    pub fn start(di: &syn::DeriveInput) -> Result<Self> {
        Ok(Core {
            ident: di.ident.clone(),
            generics: di.generics.clone(),
            data: Data::try_empty_from(&di.data)?,
            default: Default::default(),
            // See https://github.com/TedDriggs/darling/issues/10: We default to snake_case
            // for enums to help authors produce more idiomatic APIs.
            rename_rule: if let syn::Data::Enum(_) = di.data {
                RenameRule::SnakeCase
            } else {
                Default::default()
            },
            post_transform: Default::default(),
            bound: Default::default(),
            allow_unknown_fields: Default::default(),
        })
    }

    fn as_codegen_default(&self) -> Option<codegen::DefaultExpression<'_>> {
        self.default.as_ref().map(|expr| match *expr {
            DefaultExpression::Explicit(ref path) => codegen::DefaultExpression::Explicit(path),
            DefaultExpression::Inherit => {
                // It should be impossible for any input to get here,
                // so panic rather than returning an error or pretending
                // everything is fine.
                panic!("DefaultExpression::Inherit is not valid at container level")
            }
            DefaultExpression::Trait { span } => codegen::DefaultExpression::Trait { span },
        })
    }
}

impl ParseAttribute for Core {
    fn parse_nested(&mut self, mi: &syn::Meta) -> Result<()> {
        let path = mi.path();

        if path.is_ident("default") {
            if self.default.is_some() {
                return Err(Error::duplicate_field("default").with_span(mi));
            }

            self.default = FromMeta::from_meta(mi)?;
        } else if path.is_ident("rename_all") {
            // WARNING: This may have been set based on body shape previously,
            // so an overwrite may be permissible.
            self.rename_rule = FromMeta::from_meta(mi)?;
        } else if path.is_ident("map") || path.is_ident("and_then") {
            // This unwrap is safe because we just called is_ident above
            let transformer = path.get_ident().unwrap().clone();

            if let Some(post_transform) = &self.post_transform {
                if transformer == post_transform.transformer {
                    return Err(Error::duplicate_field(&transformer.to_string()).with_span(mi));
                } else {
                    return Err(Error::custom(format!(
                        "Options `{}` and `{}` are mutually exclusive",
                        transformer, post_transform.transformer
                    ))
                    .with_span(mi));
                }
            }

            self.post_transform =
                Some(PostfixTransform::new(transformer, FromMeta::from_meta(mi)?));
        } else if path.is_ident("bound") {
            self.bound = FromMeta::from_meta(mi)?;
        } else if path.is_ident("allow_unknown_fields") {
            if self.allow_unknown_fields.is_some() {
                return Err(Error::duplicate_field("allow_unknown_fields").with_span(mi));
            }

            self.allow_unknown_fields = FromMeta::from_meta(mi)?;
        } else {
            return Err(Error::unknown_field_path(path).with_span(mi));
        }

        Ok(())
    }
}

impl ParseData for Core {
    fn parse_variant(&mut self, variant: &syn::Variant) -> Result<()> {
        let v = InputVariant::from_variant(variant, Some(self))?;

        match self.data {
            Data::Enum(ref mut variants) => {
                variants.push(v);
                Ok(())
            }
            Data::Struct(_) => panic!("Core::parse_variant should never be called for a struct"),
        }
    }

    fn parse_field(&mut self, field: &syn::Field) -> Result<()> {
        let f = InputField::from_field(field, Some(self))?;

        match self.data {
            Data::Struct(Fields {
                style: Style::Unit, ..
            }) => panic!("Core::parse_field should not be called on unit"),
            Data::Struct(Fields { ref mut fields, .. }) => {
                fields.push(f);
                Ok(())
            }
            Data::Enum(_) => panic!("Core::parse_field should never be called for an enum"),
        }
    }

    fn validate_body(&self, errors: &mut Accumulator) {
        if let Data::Struct(fields) = &self.data {
            let flatten_targets: Vec<_> = fields
                .iter()
                .filter_map(|field| {
                    if field.flatten.is_present() {
                        Some(field.flatten)
                    } else {
                        None
                    }
                })
                .collect();

            if flatten_targets.len() > 1 {
                for flatten in flatten_targets {
                    errors.push(
                        Error::custom("`#[darling(flatten)]` can only be applied to one field")
                            .with_span(&flatten.span()),
                    );
                }
            }
        }
    }
}

impl<'a> From<&'a Core> for codegen::TraitImpl<'a> {
    fn from(v: &'a Core) -> Self {
        codegen::TraitImpl {
            ident: &v.ident,
            generics: &v.generics,
            data: v
                .data
                .as_ref()
                .map_struct_fields(InputField::as_codegen_field)
                .map_enum_variants(|variant| variant.as_codegen_variant(&v.ident)),
            default: v.as_codegen_default(),
            post_transform: v.post_transform.as_ref(),
            allow_unknown_fields: v.allow_unknown_fields.unwrap_or_default(),
        }
    }
}
