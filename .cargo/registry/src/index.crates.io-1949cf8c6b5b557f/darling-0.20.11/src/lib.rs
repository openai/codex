//! # Darling
//! Darling is a tool for declarative attribute parsing in proc macro implementations.
//!
//!
//! ## Design
//! Darling takes considerable design inspiration from [`serde`](https://serde.rs). A data structure that can be
//! read from any attribute implements `FromMeta` (or has an implementation automatically
//! generated using `derive`). Any crate can provide `FromMeta` implementations, even one not
//! specifically geared towards proc-macro authors.
//!
//! Proc-macro crates should provide their own structs which implement or derive `FromDeriveInput`,
//! `FromField`, `FromVariant`, `FromGenerics`, _et alia_ to gather settings relevant to their operation.
//!
//! ## Attributes
//! There are a number of attributes that `darling` exposes to enable finer-grained control over the code
//! it generates.
//!
//! * **Field renaming**: You can use `#[darling(rename="new_name")]` on a field to change the name Darling looks for.
//!   You can also use `#[darling(rename_all="...")]` at the struct or enum level to apply a casing rule to all fields or variants.
//! * **Map function**: You can use `#[darling(map="path::to::function")]` to run code on a field before its stored in the struct.
//! * **Default values**: You can use `#[darling(default)]` at the type or field level to use that type's default value to fill
//!   in values not specified by the caller.
//! * **Skipped fields**: You can skip a variant or field using `#[darling(skip)]`. Fields marked with this will fall back to
//!   `Default::default()` for their value, but you can override that with an explicit default or a value from the type-level default.
//! * **Custom shorthand**: Use `#[darling(from_word = ...)]` on a struct or enum to override how a simple word is interpreted.
//!   By default, it is an error for your macro's user to fail to specify the fields of your struct, but with this you can choose to
//!   instead produce a set of default values. This takes either a path or a closure whose signature matches `FromMeta::from_word`.
//! * **Custom handling for missing fields**: When a field is not present and `#[darling(default)]` is not used, derived impls will
//!   call `FromMeta::from_none` on that field's type to try and get the fallback value for the field. Usually, there is not a fallback
//!   value, so a missing field error is generated. `Option<T: FromMeta>` uses this to make options optional without requiring
//!   `#[darling(default)]` declarations, and structs and enums can use this themselves with `#[darling(from_none = ...)]`.
//!   This takes either a path or a closure whose signature matches `FromMeta::from_none`.
//!
//! ## Forwarded Fields
//! All derivable traits except `FromMeta` support forwarding some fields from the input AST to the derived struct.
//! These fields are matched up by identifier **before** `rename` attribute values are considered,
//! allowing you to use their names for your own properties.
//! The deriving struct is responsible for making sure the types of fields it chooses to declare are compatible with this table.
//!
//! A deriving struct is free to include or exclude any of the fields below.
//!
//! ### `FromDeriveInput`
//! |Field name|Type|Meaning|
//! |---|---|---|
//! |`ident`|`syn::Ident`|The identifier of the passed-in type|
//! |`vis`|`syn::Visibility`|The visibility of the passed-in type|
//! |`generics`|`T: darling::FromGenerics`|The generics of the passed-in type. This can be `syn::Generics`, `darling::ast::Generics`, or any compatible type.|
//! |`data` (or anything, using `#[darling(with = ...)]`)|`darling::ast::Data`|The body of the passed-in type|
//! |`attrs`|`Vec<syn::Attribute>` (or anything, using `#[darling(with = ...)]`)|The forwarded attributes from the passed in type. These are controlled using the `forward_attrs` attribute.|
//!
//! ### `FromField`
//! |Field name|Type|Meaning|
//! |---|---|---|
//! |`ident`|`Option<syn::Ident>`|The identifier of the passed-in field, or `None` for tuple fields|
//! |`vis`|`syn::Visibility`|The visibility of the passed-in field|
//! |`ty`|`syn::Type`|The type of the passed-in field|
//! |`attrs`|`Vec<syn::Attribute>` (or anything, using `#[darling(with = ...)]`)|The forwarded attributes from the passed in field. These are controlled using the `forward_attrs` attribute.|
//!
//! ### `FromTypeParam`
//! |Field name|Type|Meaning|
//! |---|---|---|
//! |`ident`|`syn::Ident`|The identifier of the passed-in type param|
//! |`bounds`|`Vec<syn::TypeParamBound>`|The bounds applied to the type param|
//! |`default`|`Option<syn::Type>`|The default type of the parameter, if one exists|
//! |`attrs`|`Vec<syn::Attribute>` (or anything, using `#[darling(with = ...)]`)|The forwarded attributes from the passed in type param. These are controlled using the `forward_attrs` attribute.|
//!
//! ### `FromVariant`
//! |Field name|Type|Meaning|
//! |---|---|---|
//! |`ident`|`syn::Ident`|The identifier of the passed-in variant|
//! |`discriminant`|`Option<syn::Expr>`|For a variant such as `Example = 2`, the `2`|
//! |`fields`|`darling::ast::Fields<T> where T: FromField`|The fields associated with the variant|
//! |`attrs`|`Vec<syn::Attribute>` (or anything, using `#[darling(with = ...)]`)|The forwarded attributes from the passed in variant. These are controlled using the `forward_attrs` attribute.|
#![warn(rust_2018_idioms)]

#[allow(unused_imports)]
#[macro_use]
extern crate darling_macro;

#[doc(hidden)]
pub use darling_macro::*;

#[doc(inline)]
pub use darling_core::{
    FromAttributes, FromDeriveInput, FromField, FromGenericParam, FromGenerics, FromMeta,
    FromTypeParam, FromVariant,
};

#[doc(inline)]
pub use darling_core::{Error, Result};

#[doc(inline)]
pub use darling_core::{ast, error, usage, util};

// XXX exported so that `ExtractAttribute::extractor` can convert a path into tokens.
// This is likely to change in the future, so only generated code should depend on this export.
#[doc(hidden)]
pub use darling_core::ToTokens;

/// Core/std trait re-exports. This should help produce generated code which doesn't
/// depend on `std` unnecessarily, and avoids problems caused by aliasing `std` or any
/// of the referenced types.
#[doc(hidden)]
pub mod export {
    pub use core::convert::{identity, From};
    pub use core::default::Default;
    pub use core::option::Option::{self, None, Some};
    pub use core::result::Result::{self, Err, Ok};
    pub use darling_core::syn;
    pub use std::string::ToString;
    pub use std::vec::Vec;

    pub use crate::ast::NestedMeta;
}

#[macro_use]
mod macros_public;
