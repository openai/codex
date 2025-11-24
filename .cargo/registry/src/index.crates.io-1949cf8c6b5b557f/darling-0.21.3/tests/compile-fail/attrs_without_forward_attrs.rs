use darling::FromDeriveInput;
use syn::{Attribute, Ident};

#[derive(FromDeriveInput)]
struct HelloArgs {
    ident: Ident,
    attrs: Vec<Attribute>,
}

fn main() {}
