//! Test that skipped fields are not read into structs when they appear in input.

use darling::{FromDeriveInput, FromMeta};
use syn::parse_quote;

#[derive(Debug, PartialEq, Eq, FromDeriveInput)]
#[darling(attributes(skip_test))]
pub struct Lorem {
    ipsum: String,

    #[darling(skip)]
    dolor: u8,
}

/// Verify variant-level and field-level skip work correctly for enums.
#[derive(Debug, FromMeta)]
pub enum Sit {
    Amet(bool),

    #[darling(skip)]
    Foo {
        hello: bool,
    },

    Bar {
        hello: bool,
        #[darling(skip)]
        world: u8,
    },
}

#[test]
fn verify_skipped_field_not_required() {
    let di = parse_quote! {
        #[skip_test(ipsum = "Hello")]
        struct Baz;
    };

    assert_eq!(
        Lorem::from_derive_input(&di).unwrap(),
        Lorem {
            ipsum: "Hello".to_string(),
            dolor: 0,
        }
    );
}

/// This test verifies that a skipped field will still prefer an explicit default
/// over the default that would come from its field type. It would be incorrect for
/// `Defaulting::from_derive_input` to fail here, and it would be wrong for the value
/// of `dolor` to be `None`.
#[test]
fn verify_default_supersedes_from_none() {
    fn default_dolor() -> Option<u8> {
        Some(2)
    }

    #[derive(Debug, PartialEq, Eq, FromDeriveInput)]
    #[darling(attributes(skip_test))]
    pub struct Defaulting {
        #[darling(skip, default = "default_dolor")]
        dolor: Option<u8>,
    }

    let di = parse_quote! {
        #[skip_test]
        struct Baz;
    };

    assert_eq!(
        Defaulting::from_derive_input(&di).unwrap(),
        Defaulting { dolor: Some(2) }
    )
}
