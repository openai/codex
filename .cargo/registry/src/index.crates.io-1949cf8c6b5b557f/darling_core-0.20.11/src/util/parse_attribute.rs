use crate::{Error, Result};
use std::fmt;
use syn::punctuated::Pair;
use syn::spanned::Spanned;
use syn::{token, Attribute, Meta, MetaList, Path};

/// Try to parse an attribute into a meta list. Path-type meta values are accepted and returned
/// as empty lists with their passed-in path. Name-value meta values and non-meta attributes
/// will cause errors to be returned.
pub fn parse_attribute_to_meta_list(attr: &Attribute) -> Result<MetaList> {
    match &attr.meta {
        Meta::List(list) => Ok(list.clone()),
        Meta::NameValue(nv) => Err(Error::custom(format!(
            "Name-value arguments are not supported. Use #[{}(...)]",
            DisplayPath(&nv.path)
        ))
        .with_span(&nv)),
        Meta::Path(path) => Ok(MetaList {
            path: path.clone(),
            delimiter: syn::MacroDelimiter::Paren(token::Paren {
                span: {
                    let mut group = proc_macro2::Group::new(
                        proc_macro2::Delimiter::None,
                        proc_macro2::TokenStream::new(),
                    );
                    group.set_span(attr.span());
                    group.delim_span()
                },
            }),
            tokens: Default::default(),
        }),
    }
}

struct DisplayPath<'a>(&'a Path);

impl fmt::Display for DisplayPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self.0;
        if path.leading_colon.is_some() {
            write!(f, "::")?;
        }
        for segment in path.segments.pairs() {
            match segment {
                Pair::Punctuated(segment, _) => write!(f, "{}::", segment.ident)?,
                Pair::End(segment) => segment.ident.fmt(f)?,
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_attribute_to_meta_list;
    use crate::ast::NestedMeta;
    use syn::spanned::Spanned;
    use syn::{parse_quote, Ident};

    #[test]
    fn parse_list() {
        let meta = parse_attribute_to_meta_list(&parse_quote!(#[bar(baz = 4)])).unwrap();
        let nested_meta = NestedMeta::parse_meta_list(meta.tokens).unwrap();
        assert_eq!(nested_meta.len(), 1);
    }

    #[test]
    fn parse_path_returns_empty_list() {
        let meta = parse_attribute_to_meta_list(&parse_quote!(#[bar])).unwrap();
        let nested_meta = NestedMeta::parse_meta_list(meta.tokens).unwrap();
        assert!(meta.path.is_ident(&Ident::new("bar", meta.path.span())));
        assert!(nested_meta.is_empty());
    }

    #[test]
    fn parse_name_value_returns_error() {
        parse_attribute_to_meta_list(&parse_quote!(#[bar = 4])).unwrap_err();
    }

    #[test]
    fn parse_name_value_error_includes_example() {
        let err = parse_attribute_to_meta_list(&parse_quote!(#[bar = 4])).unwrap_err();
        assert!(err.to_string().contains("#[bar(...)]"));
    }
}
