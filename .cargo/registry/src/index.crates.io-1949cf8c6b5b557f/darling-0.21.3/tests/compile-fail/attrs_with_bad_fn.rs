use darling::FromDeriveInput;
use syn::Attribute;

fn bad_converter(attrs: Vec<Attribute>) -> Vec<Attribute> {
    attrs
}

#[derive(FromDeriveInput)]
#[darling(forward_attrs)]
struct Receiver {
    #[darling(with = bad_converter)]
    attrs: Vec<Attribute>,
}

fn main() {}
