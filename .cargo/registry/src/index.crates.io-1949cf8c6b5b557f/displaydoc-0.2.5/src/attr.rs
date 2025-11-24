use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{Attribute, LitStr, Meta, Result};

#[derive(Clone)]
pub(crate) struct Display {
    pub(crate) fmt: LitStr,
    pub(crate) args: TokenStream,
}

pub(crate) struct VariantDisplay {
    pub(crate) r#enum: Option<Display>,
    pub(crate) variant: Display,
}

impl ToTokens for Display {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let fmt = &self.fmt;
        let args = &self.args;
        tokens.extend(quote! {
            write!(formatter, #fmt #args)
        });
    }
}

impl ToTokens for VariantDisplay {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some(ref r#enum) = self.r#enum {
            r#enum.to_tokens(tokens);
            tokens.extend(quote! { ?; write!(formatter, ": ")?; });
        }
        self.variant.to_tokens(tokens);
    }
}

pub(crate) struct AttrsHelper {
    ignore_extra_doc_attributes: bool,
    prefix_enum_doc_attributes: bool,
}

impl AttrsHelper {
    pub(crate) fn new(attrs: &[Attribute]) -> Self {
        let ignore_extra_doc_attributes = attrs
            .iter()
            .any(|attr| attr.path().is_ident("ignore_extra_doc_attributes"));
        let prefix_enum_doc_attributes = attrs
            .iter()
            .any(|attr| attr.path().is_ident("prefix_enum_doc_attributes"));

        Self {
            ignore_extra_doc_attributes,
            prefix_enum_doc_attributes,
        }
    }

    pub(crate) fn display(&self, attrs: &[Attribute]) -> Result<Option<Display>> {
        let displaydoc_attr = attrs.iter().find(|attr| attr.path().is_ident("displaydoc"));

        if let Some(displaydoc_attr) = displaydoc_attr {
            let lit = displaydoc_attr
                .parse_args()
                .expect("#[displaydoc(\"foo\")] must contain string arguments");
            let mut display = Display {
                fmt: lit,
                args: TokenStream::new(),
            };

            display.expand_shorthand();
            return Ok(Some(display));
        }

        let num_doc_attrs = attrs
            .iter()
            .filter(|attr| attr.path().is_ident("doc"))
            .count();

        if !self.ignore_extra_doc_attributes && num_doc_attrs > 1 {
            panic!("Multi-line comments are disabled by default by displaydoc. Please consider using block doc comments (/** */) or adding the #[ignore_extra_doc_attributes] attribute to your type next to the derive.");
        }

        for attr in attrs {
            if attr.path().is_ident("doc") {
                let lit = match &attr.meta {
                    Meta::NameValue(syn::MetaNameValue {
                        value:
                            syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Str(lit),
                                ..
                            }),
                        ..
                    }) => lit,
                    _ => unimplemented!(),
                };

                // Make an attempt at cleaning up multiline doc comments.
                let doc_str = lit
                    .value()
                    .lines()
                    .map(|line| line.trim().trim_start_matches('*').trim())
                    .collect::<Vec<&str>>()
                    .join("\n");

                let lit = LitStr::new(doc_str.trim(), lit.span());

                let mut display = Display {
                    fmt: lit,
                    args: TokenStream::new(),
                };

                display.expand_shorthand();
                return Ok(Some(display));
            }
        }

        Ok(None)
    }

    pub(crate) fn display_with_input(
        &self,
        r#enum: &[Attribute],
        variant: &[Attribute],
    ) -> Result<Option<VariantDisplay>> {
        let r#enum = if self.prefix_enum_doc_attributes {
            let result = self
                .display(r#enum)?
                .expect("Missing doc comment on enum with #[prefix_enum_doc_attributes]. Please remove the attribute or add a doc comment to the enum itself.");

            Some(result)
        } else {
            None
        };

        Ok(self
            .display(variant)?
            .map(|variant| VariantDisplay { r#enum, variant }))
    }
}
