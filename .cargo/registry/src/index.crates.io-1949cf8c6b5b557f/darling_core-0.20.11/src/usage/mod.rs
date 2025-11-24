//! Traits and types used for tracking the usage of generic parameters through a proc-macro input.
//!
//! When generating trait impls, libraries often want to automatically figure out which type parameters
//! are used in which fields, and then emit bounds that will produce the most permissive compilable
//! code.
//!
//! # Usage
//!
//! ## Example 1: Filtering
//! This example accepts a proc-macro input, then finds all lifetimes and type parameters used
//! by private fields.
//!
//! ```rust
//! # extern crate darling_core;
//! # extern crate syn;
//! #
//! # // in real-world usage, import from `darling`
//! # use darling_core::usage::{self, CollectLifetimes, CollectTypeParams, GenericsExt, Purpose};
//! # use syn::{Data, DeriveInput, GenericParam, Generics, Visibility};
//! #
//! # #[allow(dead_code)]
//! fn process(input: &DeriveInput) -> Generics {
//!     let type_params = input.generics.declared_type_params();
//!     let lifetimes = input.generics.declared_lifetimes();
//!
//!     let mut ret_generics = input.generics.clone();
//!
//!     if let Data::Struct(ref body) = input.data {
//!         let internal_fields = body
//!             .fields
//!             .iter()
//!             .filter(|field| field.vis == Visibility::Inherited)
//!             .collect::<Vec<_>>();
//!
//!         let int_type_params = internal_fields
//!             .collect_type_params(&Purpose::BoundImpl.into(), &type_params);
//!
//!         // We could reuse the vec from above, but here we'll instead
//!         // directly consume the chained iterator.
//!         let int_lifetimes = body
//!             .fields
//!             .iter()
//!             .filter(|field| field.vis == Visibility::Inherited)
//!             .collect_lifetimes(&Purpose::BoundImpl.into(), &lifetimes);
//!
//!
//!         ret_generics.params = ret_generics
//!             .params
//!             .into_iter()
//!             .filter(|gp| {
//!                 match *gp {
//!                     GenericParam::Type(ref ty) => int_type_params.contains(&ty.ident),
//!                     GenericParam::Lifetime(ref lt) => int_lifetimes.contains(&lt.lifetime),
//!                     _ => true,
//!                 }
//!             })
//!             .collect();
//!     }
//!
//!     ret_generics
//! }
//!
//! # fn main() {}
//! ```
//!
//! ## Example 2: Integrating with `FromDeriveInput`
//! It is possible to use `darling`'s magic fields feature in tandem with the `usage` feature set.
//! While there is no custom derive for `UsesTypeParams` or `UsesLifetimes`, there are macros to
//! generate impls.
//!
//! ```rust,ignore
//! #![allow(dead_code)]
//!
//! #[derive(FromField)]
//! #[darling(attributes(speak))]
//! struct SpeakerField {
//!     ident: Option<syn::Ident>,
//!     ty: syn::Type,
//!     #[darling(default)]
//!     volume: Option<u32>,
//! }
//!
//! uses_type_params!(SpeakerField, ty);
//! uses_lifetimes!(SpeakerField, ty);
//!
//! #[derive(FromDeriveInput)]
//! struct SpeakerOptions {
//!     generics: syn::Generics,
//!     data: darling::ast::Data<darling::util::Ignored, SpeakerField>,
//! }
//! ```
//!
//! At this point, you are able to call `uses_type_params` on `SpeakerOptions.data`, or any filtered
//! view of it. `darling` internally uses this in conjunction with the `skip` meta-item to determine
//! which type parameters don't require the `FromMeta` bound in generated impls.
//!
//! **Note:** If you are performing operations referencing generic params in meta-items parsed by `darling`,
//! you should determine if those impact the emitted code and wire up `UsesTypeParams` accordingly for
//! your field/variant.

mod generics_ext;
mod ident_set;
mod lifetimes;
mod options;
mod type_params;

pub use self::generics_ext::GenericsExt;
pub use self::ident_set::{IdentRefSet, IdentSet};
pub use self::lifetimes::{CollectLifetimes, LifetimeRefSet, LifetimeSet, UsesLifetimes};
pub use self::options::{Options, Purpose};
pub use self::type_params::{CollectTypeParams, UsesTypeParams};
