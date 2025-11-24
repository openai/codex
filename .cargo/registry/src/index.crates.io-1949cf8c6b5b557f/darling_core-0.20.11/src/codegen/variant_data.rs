use proc_macro2::TokenStream;
use quote::quote;

use crate::ast::{Fields, Style};
use crate::codegen::Field;

pub struct FieldsGen<'a> {
    fields: &'a Fields<Field<'a>>,
    allow_unknown_fields: bool,
}

impl<'a> FieldsGen<'a> {
    pub fn new(fields: &'a Fields<Field<'a>>, allow_unknown_fields: bool) -> Self {
        Self {
            fields,
            allow_unknown_fields,
        }
    }

    /// Create declarations for all the fields in the struct.
    pub(in crate::codegen) fn declarations(&self) -> TokenStream {
        match *self.fields {
            Fields {
                style: Style::Struct,
                ref fields,
                ..
            } => {
                let vdr = fields.iter().map(Field::as_declaration);
                quote!(#(#vdr)*)
            }
            _ => panic!("FieldsGen doesn't support tuples yet"),
        }
    }

    /// Generate the loop which walks meta items looking for property matches.
    pub(in crate::codegen) fn core_loop(&self) -> TokenStream {
        let arms = self.fields.as_ref().map(Field::as_match);
        // If there is a flatten field, buffer the unknown field so it can be passed
        // to the flatten function with all other unknown fields.
        let handle_unknown = if self.fields.iter().any(|f| f.flatten) {
            quote! {
                __flatten.push(::darling::ast::NestedMeta::Meta(__inner.clone()));
            }
        }
        // If we're allowing unknown fields, then handling one is a no-op.
        else if self.allow_unknown_fields {
            quote!()
        }
        // Otherwise, we're going to push a new spanned error pointing at the field.
        else {
            let mut names = self.fields.iter().filter_map(Field::as_name).peekable();
            // We can't call `unknown_field_with_alts` with an empty slice, or else it fails to
            // infer the type of the slice item.
            let err_fn = if names.peek().is_none() {
                quote!(unknown_field(__other))
            } else {
                quote!(unknown_field_with_alts(__other, &[#(#names),*]))
            };

            quote! {
                __errors.push(::darling::Error::#err_fn.with_span(__inner));
            }
        };
        let arms = arms.iter();

        quote!(
            for __item in __items {
                match *__item {
                    ::darling::export::NestedMeta::Meta(ref __inner) => {
                        let __name = ::darling::util::path_to_string(__inner.path());
                        match __name.as_str() {
                            #(#arms)*
                            __other => { #handle_unknown }
                        }
                    }
                    ::darling::export::NestedMeta::Lit(ref __inner) => {
                        __errors.push(::darling::Error::unsupported_format("literal")
                            .with_span(__inner));
                    }
                }
            }
        )
    }

    pub fn require_fields(&self) -> TokenStream {
        match *self.fields {
            Fields {
                style: Style::Struct,
                ref fields,
                ..
            } => {
                let checks = fields.iter().map(Field::as_presence_check);
                quote!(#(#checks)*)
            }
            _ => panic!("FieldsGen doesn't support tuples for requirement checks"),
        }
    }

    pub(in crate::codegen) fn initializers(&self) -> TokenStream {
        let inits = self.fields.as_ref().map(Field::as_initializer);
        let inits = inits.iter();

        quote!(#(#inits),*)
    }
}
