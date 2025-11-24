use proc_macro2::TokenStream;

#[derive(Debug, Clone)]
pub struct SchemaMetadata<'a> {
    pub title: Option<&'a str>,
    pub description: Option<&'a str>,
    pub deprecated: bool,
    pub read_only: bool,
    pub write_only: bool,
    pub examples: &'a [syn::Path],
    pub default: Option<TokenStream>,
}

impl<'a> SchemaMetadata<'a> {
    pub fn apply_to_schema(&self, schema_expr: &mut TokenStream) {
        if let Some(title) = &self.title {
            *schema_expr = quote! {
                schemars::_private::metadata::add_title(#schema_expr, #title)
            };
        }
        if let Some(description) = &self.description {
            *schema_expr = quote! {
                schemars::_private::metadata::add_description(#schema_expr, #description)
            };
        }

        if self.deprecated {
            *schema_expr = quote! {
                schemars::_private::metadata::add_deprecated(#schema_expr, true)
            };
        }

        if self.read_only {
            *schema_expr = quote! {
                schemars::_private::metadata::add_read_only(#schema_expr, true)
            };
        }
        if self.write_only {
            *schema_expr = quote! {
                schemars::_private::metadata::add_write_only(#schema_expr, true)
            };
        }

        if !self.examples.is_empty() {
            let examples = self.examples.iter().map(|eg| {
                quote! {
                    schemars::_serde_json::value::to_value(#eg())
                }
            });

            *schema_expr = quote! {
                schemars::_private::metadata::add_examples(#schema_expr,  [#(#examples),*].into_iter().flatten())
            };
        }

        if let Some(default) = &self.default {
            *schema_expr = quote! {
                schemars::_private::metadata::add_default(#schema_expr, #default.and_then(|d| schemars::_schemars_maybe_to_value!(d)))
            };
        }
    }
}
