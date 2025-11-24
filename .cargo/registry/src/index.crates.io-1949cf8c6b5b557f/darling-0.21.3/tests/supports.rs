use darling::{ast, FromDeriveInput, FromField, FromVariant};

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(from_variants), supports(enum_any))]
pub struct Container {
    // The second type parameter can be anything that implements FromField, since
    // FromDeriveInput will produce an error if given a struct.
    data: ast::Data<Variant, Panic>,
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
    data: ast::Data<Panic, syn::Field>,
}

/// A struct that will panic if construction is attempted via `FromVariant` or `FromField`.
///
/// These tests use this to ensure no attempts are made to read fields or variants if
/// shape validation has failed. Failure to do this could cause panics or spurious errors
/// to be emitted by derived `FromDeriveInput` impls, which breaks library author's trust
/// in `darling` to emit great error messages.
#[derive(Debug)]
struct Panic;

impl FromVariant for Panic {
    fn from_variant(variant: &syn::Variant) -> darling::Result<Self> {
        panic!("Should not have called from_variant on {}", variant.ident);
    }
}

impl FromField for Panic {
    fn from_field(_field: &syn::Field) -> darling::Result<Self> {
        panic!("Should not have called from_field");
    }
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
