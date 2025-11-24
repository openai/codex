use darling::{ast, FromDeriveInput, FromVariant};

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(from_variants), supports(enum_any))]
pub struct Container {
    // The second type parameter can be anything that implements FromField, since
    // FromDeriveInput will produce an error if given a struct.
    data: ast::Data<Variant, ()>,
}

#[derive(Default, Debug, FromVariant)]
#[darling(default, attributes(from_variants), supports(newtype, unit))]
pub struct Variant {
    into: Option<bool>,
    skip: Option<bool>,
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(from_struct), supports(struct_named))]
pub struct StructContainer {
    // The second type parameter can be anything that implements FromVariant, since
    // FromDeriveInput will produce an error if given an enum.
    data: ast::Data<(), syn::Field>,
}

mod source {
    use syn::{parse_quote, DeriveInput};

    pub fn newtype_enum() -> DeriveInput {
        parse_quote! {
            enum Hello {
                World(bool),
                String(String),
            }
        }
    }

    pub fn named_field_enum() -> DeriveInput {
        parse_quote! {
            enum Hello {
                Foo(u16),
                World {
                    name: String
                },
            }
        }
    }

    pub fn empty_enum() -> DeriveInput {
        parse_quote! {
            enum Hello {}
        }
    }

    pub fn named_struct() -> DeriveInput {
        parse_quote! {
            struct Hello {
                world: bool,
            }
        }
    }

    pub fn tuple_struct() -> DeriveInput {
        parse_quote! { struct Hello(String, bool); }
    }
}

#[test]
fn enum_newtype_or_unit() {
    // Should pass
    let container = Container::from_derive_input(&source::newtype_enum()).unwrap();
    assert!(container.data.is_enum());

    // Should error
    Container::from_derive_input(&source::named_field_enum()).unwrap_err();
    Container::from_derive_input(&source::named_struct()).unwrap_err();
}

#[test]
fn struct_named() {
    // Should pass
    let container = StructContainer::from_derive_input(&source::named_struct()).unwrap();
    assert!(container.data.is_struct());

    // Should fail
    StructContainer::from_derive_input(&source::tuple_struct()).unwrap_err();
    StructContainer::from_derive_input(&source::named_field_enum()).unwrap_err();
    StructContainer::from_derive_input(&source::newtype_enum()).unwrap_err();
    StructContainer::from_derive_input(&source::empty_enum()).unwrap_err();
}
