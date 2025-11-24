#[derive(darling::FromDeriveInput)]
#[darling(attributes(example), forward_attrs(example))]
pub struct Example {
    ignored: String,
}

#[derive(darling::FromField)]
#[darling(attributes(example), forward_attrs(example))]
pub struct ExampleField {
    ignored: String,
}

fn main() {}
