use crate::utils::{
    field_idents, get_field_types, named_to_vec, numbered_vars, unnamed_to_vec,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Field, Fields, Ident};

/// Provides the hook to expand `#[derive(Constructor)]` into an implementation of `Constructor`
pub fn expand(input: &DeriveInput, _: &str) -> TokenStream {
    let input_type = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let ((body, vars), fields) = match input.data {
        Data::Struct(ref data_struct) => match data_struct.fields {
            Fields::Unnamed(ref fields) => {
                let field_vec = unnamed_to_vec(fields);
                (tuple_body(input_type, &field_vec), field_vec)
            }
            Fields::Named(ref fields) => {
                let field_vec = named_to_vec(fields);
                (struct_body(input_type, &field_vec), field_vec)
            }
            Fields::Unit => (struct_body(input_type, &[]), vec![]),
        },
        _ => panic!("Only structs can derive a constructor"),
    };
    let original_types = &get_field_types(&fields);
    quote! {
        #[allow(missing_docs)]
        #[automatically_derived]
        impl #impl_generics #input_type #ty_generics #where_clause {
            #[inline]
            pub const fn new(#(#vars: #original_types),*) -> #input_type #ty_generics {
                #body
            }
        }
    }
}

fn tuple_body(return_type: &Ident, fields: &[&Field]) -> (TokenStream, Vec<Ident>) {
    let vars = &numbered_vars(fields.len(), "");
    (quote! { #return_type(#(#vars),*) }, vars.clone())
}

fn struct_body(return_type: &Ident, fields: &[&Field]) -> (TokenStream, Vec<Ident>) {
    let field_names: &Vec<Ident> =
        &field_idents(fields).iter().map(|f| (**f).clone()).collect();
    let vars = field_names;
    let ret_vars = field_names.clone();
    (quote! { #return_type{#(#field_names: #vars),*} }, ret_vars)
}
