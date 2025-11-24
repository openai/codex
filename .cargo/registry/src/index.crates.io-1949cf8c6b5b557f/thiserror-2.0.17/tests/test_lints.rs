#![allow(clippy::mixed_attributes_style)]

use thiserror::Error;

pub use std::error::Error;

#[test]
fn test_allow_attributes() {
    #![deny(clippy::allow_attributes)]

    #[derive(Error, Debug)]
    #[error("...")]
    pub struct MyError(#[from] anyhow::Error);

    let _: MyError;
}

#[test]
fn test_unused_qualifications() {
    #![deny(unused_qualifications)]

    // Expansion of derive(Error) macro can't know whether something like
    // std::error::Error is already imported in the caller's scope so it must
    // suppress unused_qualifications.

    #[derive(Error, Debug)]
    #[error("...")]
    pub struct MyError;

    let _: MyError;
}

#[test]
fn test_needless_lifetimes() {
    #![allow(dead_code)]
    #![deny(clippy::elidable_lifetime_names, clippy::needless_lifetimes)]

    #[derive(Error, Debug)]
    #[error("...")]
    pub enum MyError<'a> {
        A(#[from] std::io::Error),
        B(&'a ()),
    }

    let _: MyError;
}

#[test]
fn test_deprecated() {
    #![deny(deprecated)]

    #[derive(Error, Debug)]
    #[deprecated]
    #[error("...")]
    pub struct DeprecatedStruct;

    #[derive(Error, Debug)]
    #[error("{message} {}", .message)]
    pub struct DeprecatedStructField {
        #[deprecated]
        message: String,
    }

    #[derive(Error, Debug)]
    #[deprecated]
    pub enum DeprecatedEnum {
        #[error("...")]
        Variant,
    }

    #[derive(Error, Debug)]
    pub enum DeprecatedVariant {
        #[deprecated]
        #[error("...")]
        Variant,
    }

    #[derive(Error, Debug)]
    pub enum DeprecatedFrom {
        #[error(transparent)]
        Variant(
            #[from]
            #[allow(deprecated)]
            DeprecatedStruct,
        ),
    }

    #[allow(deprecated)]
    let _: DeprecatedStruct;
    #[allow(deprecated)]
    let _: DeprecatedStructField;
    #[allow(deprecated)]
    let _ = DeprecatedEnum::Variant;
    #[allow(deprecated)]
    let _ = DeprecatedVariant::Variant;
    #[allow(deprecated)]
    let _ = DeprecatedFrom::Variant(DeprecatedStruct);
}
