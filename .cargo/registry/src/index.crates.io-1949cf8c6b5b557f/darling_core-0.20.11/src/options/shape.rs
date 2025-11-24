//! Types for "shape" validation. This allows types deriving `FromDeriveInput` etc. to declare
//! that they only work on - for example - structs with named fields, or newtype enum variants.

use proc_macro2::TokenStream;
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{parse_quote, Meta};

use crate::ast::NestedMeta;
use crate::{Error, FromMeta, Result};

/// Receiver struct for shape validation. Shape validation allows a deriving type
/// to declare that it only accepts - for example - named structs, or newtype enum
/// variants.
///
/// ```rust,ignore
/// #[ignore(any, struct_named, enum_newtype)]
/// ```
#[derive(Debug, Clone)]
pub struct DeriveInputShapeSet {
    enum_values: DataShape,
    struct_values: DataShape,
    any: bool,
}

impl Default for DeriveInputShapeSet {
    fn default() -> Self {
        DeriveInputShapeSet {
            enum_values: DataShape::new("enum_"),
            struct_values: DataShape::new("struct_"),
            any: Default::default(),
        }
    }
}

impl FromMeta for DeriveInputShapeSet {
    fn from_list(items: &[NestedMeta]) -> Result<Self> {
        let mut new = DeriveInputShapeSet::default();
        for item in items {
            if let NestedMeta::Meta(Meta::Path(ref path)) = *item {
                let ident = &path.segments.first().unwrap().ident;
                let word = ident.to_string();
                if word == "any" {
                    new.any = true;
                } else if word.starts_with("enum_") {
                    new.enum_values
                        .set_word(&word)
                        .map_err(|e| e.with_span(&ident))?;
                } else if word.starts_with("struct_") {
                    new.struct_values
                        .set_word(&word)
                        .map_err(|e| e.with_span(&ident))?;
                } else {
                    return Err(Error::unknown_value(&word).with_span(&ident));
                }
            } else {
                return Err(Error::unsupported_format("non-word").with_span(item));
            }
        }

        Ok(new)
    }
}

impl ToTokens for DeriveInputShapeSet {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let fn_body = if self.any {
            quote!(::darling::export::Ok(()))
        } else {
            let en = &self.enum_values;
            let st = &self.struct_values;

            quote! {
                {
                    let struct_check = #st;
                    let enum_check = #en;

                    match *__body {
                        ::darling::export::syn::Data::Enum(ref data) => {
                            if enum_check.is_empty() {
                                return ::darling::export::Err(
                                    ::darling::Error::unsupported_shape_with_expected("enum", &format!("struct with {}", struct_check))
                                );
                            }

                            let mut variant_errors = ::darling::Error::accumulator();
                            for variant in &data.variants {
                                variant_errors.handle(enum_check.check(variant));
                            }

                            variant_errors.finish()
                        }
                        ::darling::export::syn::Data::Struct(ref struct_data) => {
                            if struct_check.is_empty() {
                                return ::darling::export::Err(
                                    ::darling::Error::unsupported_shape_with_expected("struct", &format!("enum with {}", enum_check))
                                );
                            }

                            struct_check.check(struct_data)
                        }
                        ::darling::export::syn::Data::Union(_) => unreachable!(),
                    }
                }
            }
        };

        tokens.append_all(quote! {
            #[allow(unused_variables)]
            fn __validate_body(__body: &::darling::export::syn::Data) -> ::darling::Result<()> {
                #fn_body
            }
        });
    }
}

/// Receiver for shape information within a struct or enum context. See `Shape` for more information
/// on valid uses of shape validation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataShape {
    /// The kind of shape being described. This can be `struct_` or `enum_`.
    prefix: &'static str,
    newtype: bool,
    named: bool,
    tuple: bool,
    unit: bool,
    any: bool,
}

impl DataShape {
    fn new(prefix: &'static str) -> Self {
        DataShape {
            prefix,
            ..Default::default()
        }
    }

    fn set_word(&mut self, word: &str) -> Result<()> {
        match word.trim_start_matches(self.prefix) {
            "newtype" => {
                self.newtype = true;
                Ok(())
            }
            "named" => {
                self.named = true;
                Ok(())
            }
            "tuple" => {
                self.tuple = true;
                Ok(())
            }
            "unit" => {
                self.unit = true;
                Ok(())
            }
            "any" => {
                self.any = true;
                Ok(())
            }
            _ => Err(Error::unknown_value(word)),
        }
    }
}

impl FromMeta for DataShape {
    fn from_list(items: &[NestedMeta]) -> Result<Self> {
        let mut errors = Error::accumulator();
        let mut new = DataShape::default();

        for item in items {
            if let NestedMeta::Meta(Meta::Path(ref path)) = *item {
                errors.handle(new.set_word(&path.segments.first().unwrap().ident.to_string()));
            } else {
                errors.push(Error::unsupported_format("non-word").with_span(item));
            }
        }

        errors.finish_with(new)
    }
}

impl ToTokens for DataShape {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            any,
            named,
            tuple,
            unit,
            newtype,
            ..
        } = *self;

        let shape_path: syn::Path = parse_quote!(::darling::util::Shape);

        let mut shapes = vec![];
        if any || named {
            shapes.push(quote!(#shape_path::Named));
        }

        if any || tuple {
            shapes.push(quote!(#shape_path::Tuple));
        }

        if any || newtype {
            shapes.push(quote!(#shape_path::Newtype));
        }

        if any || unit {
            shapes.push(quote!(#shape_path::Unit));
        }

        tokens.append_all(quote! {
            ::darling::util::ShapeSet::new(vec![#(#shapes),*])
        });
    }
}

#[cfg(test)]
mod tests {
    use proc_macro2::TokenStream;
    use quote::quote;
    use syn::parse_quote;

    use super::DeriveInputShapeSet;
    use crate::FromMeta;

    /// parse a string as a syn::Meta instance.
    fn pm(tokens: TokenStream) -> ::std::result::Result<syn::Meta, String> {
        let attribute: syn::Attribute = parse_quote!(#[#tokens]);
        Ok(attribute.meta)
    }

    fn fm<T: FromMeta>(tokens: TokenStream) -> T {
        FromMeta::from_meta(&pm(tokens).expect("Tests should pass well-formed input"))
            .expect("Tests should pass valid input")
    }

    #[test]
    fn supports_any() {
        let decl = fm::<DeriveInputShapeSet>(quote!(ignore(any)));
        assert!(decl.any);
    }

    #[test]
    fn supports_struct() {
        let decl = fm::<DeriveInputShapeSet>(quote!(ignore(struct_any, struct_newtype)));
        assert!(decl.struct_values.any);
        assert!(decl.struct_values.newtype);
    }

    #[test]
    fn supports_mixed() {
        let decl =
            fm::<DeriveInputShapeSet>(quote!(ignore(struct_newtype, enum_newtype, enum_tuple)));
        assert!(decl.struct_values.newtype);
        assert!(decl.enum_values.newtype);
        assert!(decl.enum_values.tuple);
        assert!(!decl.struct_values.any);
    }
}
