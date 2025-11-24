use proc_macro2::TokenStream;
use quote::quote;
use syn::{Field, Ident, Index};

pub fn tuple_exprs(fields: &[&Field], method_ident: &Ident) -> Vec<TokenStream> {
    let mut exprs = vec![];

    for i in 0..fields.len() {
        let i = Index::from(i);
        // generates `self.0.add(rhs.0)`
        let expr = quote! { self.#i.#method_ident(rhs.#i) };
        exprs.push(expr);
    }
    exprs
}

pub fn struct_exprs(fields: &[&Field], method_ident: &Ident) -> Vec<TokenStream> {
    let mut exprs = vec![];

    for field in fields {
        // It's safe to unwrap because struct fields always have an identifier
        let field_id = field.ident.as_ref().unwrap();
        // generates `x: self.x.add(rhs.x)`
        let expr = quote! { self.#field_id.#method_ident(rhs.#field_id) };
        exprs.push(expr)
    }
    exprs
}
