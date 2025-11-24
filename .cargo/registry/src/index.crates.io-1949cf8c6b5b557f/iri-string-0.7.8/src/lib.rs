//! String types for [RFC 3987 Internationalized Resource Identifiers (IRIs)][RFC 3987] and
//! [RFC 3986 Uniform Resource Identifiers (URIs)][RFC 3986].
//!
//! Note that this crate does not have any extra knowledge about protocols.
//! Comparisons between IRI strings by `PartialEq` and `Eq` is implemented as [simple string
//! comparison](https://tools.ietf.org/html/rfc3986#section-6.2.1).
//! You should implement by yourself or use another crate to use such extra knowledge to compare
//! IRIs / URIs.
//!
//! # Capability
//!
//! This crate provides many features for IRIs / URIs.
//!
//! ## String types
//!
//! [`types` module][`types`] module provides various string types for IRIs and URIs.
//! The borrowed string types are unsized slice types (such as `[u8]` and `str`)
//! and not a sized struct, so they are highly interoperable with for example
//! `Cow` and `Rc`. Conversions between `&str` and borrwed IRI string types are easy.
//!
//! ## Resolvers
//!
//! [`resolve` module][`resolve`] provides IRI / URI references resolver.
//! However, you are recommended to use methods of string types such as
//! [`RiReferenceStr::resolve_against()`] or [`RiRelativeStr::resolve_against()`]
//! if you don't intend to resolve multiple IRIs against the same base.
//!
//! ## Validators
//!
//! Validator functions are provided from [`validate` module][`validate`].
//!
//! ## Percent encoding
//!
//! [`percent_encode` module][`percent_encode`] provides a converter to encode
//! user-provided string into percent-encoded one (if syntax requires so).
//!
//! ## IRI builder
//!
//! [`build` module][`build`] provides IRI builder.
//!
//! ## URI template (RFC 6570)
//!
//! [`template` module][`template`] provides an RFC 6570 URI Template processor.
//!
//! # Feature flags
//!
//! ## `std` and `alloc` support
//!
//! This crate supports `no_std` usage.
//!
//! * `alloc` feature:
//!     + Std library or `alloc` crate is required.
//!     + This feature enables types and functions which require memory allocation,
//!       e.g. `types::IriString` and `types::IriRelativeStr::resolve_against()`.
//! * `std` feature (**enabled by default**):
//!     + Std library is required.
//!     + This automatically enables `alloc` feature.
//!     + The feature let the crate utilize std-specific stuff, such as `std::error::Error` trait.
//! * With neither of them:
//!     + The crate can be used in `no_std` environment.
//!
//! ## Other features
//!
//! * `serde`
//!     + Enables serde support.
//!     + Implement `Serailize` and `Deserialize` traits for IRI / URI types.
//! * `memchr`
//!     + Enables faster internal character search.
//!
//! # Rationale
//!
//! ## `foo:`, `foo:/`, `foo://`, `foo:///`, `foo:////`, ... are valid IRIs
//!
//! All of these are valid IRIs.
//! (On the other hand, all of them are invalid as relative IRI reference, because they don't
//! match `relative-part` rule, especially `path-noscheme`, as the first path component of the
//! relative path contains a colon.)
//!
//! * `foo:`
//!     + Decomposed to `<scheme="foo">:<path-empty="">`.
//! * `foo:/`
//!     + Decomposed to `<scheme="foo">:<path-absolute="/">`.
//! * `foo://`
//!     + Decomposed to `<scheme="foo">://<authority=""><path-absolute="">`.
//! * `foo:///`
//!     + Decomposed to `<scheme="foo">://<authority=""><path-absolute="/">`.
//! * `foo:////`
//!     + Decomposed to `<scheme="foo">://<authority=""><path-absolute="//">`.
//! * `foo://///`
//!     + Decomposed to `<scheme="foo">://<authority=""><path-absolute="///">`.
//!
//! RFC 3986 says that "if authority is absent, path cannot start with `//`".
//!
//! > When authority is present, the path must either be empty or begin with a slash ("/")
//! > character. When authority is not present, the path cannot begin with two slash characters
//! > ("//").
//! >
//! > --- [RFC 3986, section 3. Syntax Components](https://tools.ietf.org/html/rfc3986#section-3).
//!
//! > If a URI contains an authority component, then the path component must either be empty or
//! > begin with a slash ("/") character. If a URI does not contain an authority component, then the
//! > path cannot begin with two slash characters ("//").
//! >
//! > --- [RFC 3986, section 3.3. Path](https://tools.ietf.org/html/rfc3986#section-3.3)
//!
//! We should interpret them as "if `authority` rule is completely unused (i.e. does not match any
//! strings **including empty string**), path cannot start with `//`".
//! In other words, we should consider this as **explaining the ABNF of `hier-part` rule**
//! (especially why it does not use `path` rule), but **not adding extra restriction to the rule
//! written in ABNF**.
//!
//! This restriction is necessary to remove ambiguity in decomposition of some strings.
//! For example, it is natural to decompose `foo://` to `<scheme="foo">:<path="//">` or
//! `<scheme="foo">://<authority=""><path="">`.
//! The restriction, **which is already encoded to the ABNF rule**, tells us to always decompose to
//! the latter form, rather than the former one.
//!
//! Readers of the spec might be confused by "when authority is **present**" and "if a URI
//! **contains** an authority component, which is unclear.
//! However, based on the interpretation above, we should consider authority part with empty string
//! as satisfying the condition "authority is **present**".
//!
//! ## IRI resolution can fail
//!
//! For some inputs, resulting string of IRI normalization and resolution can be syntactically
//! correct but semantically wrong. In such cases, the normalizer and resolver provided by this
//! crate do not silently "fix" the IRI by non-standard processing, but just
//! fail by returning `Err(_)`.
//!
//! For details, see the documentation of [`normalize`] module.
//!
//! [RFC 3986]: https://tools.ietf.org/html/rfc3986
//! [RFC 3987]: https://tools.ietf.org/html/rfc3987
//! [`RiReferenceStr::resolve_against()`]: `types::RiReferenceStr::resolve_against`
//! [`RiRelativeStr::resolve_against()`]: `types::RiRelativeStr::resolve_against`
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(clippy::missing_docs_in_private_items)]
#![warn(clippy::undocumented_unsafe_blocks)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod build;
pub mod components;
pub mod convert;
pub mod format;
pub mod mask_password;
pub mod normalize;
pub(crate) mod parser;
pub mod percent_encode;
pub(crate) mod raw;
pub mod resolve;
pub mod spec;
pub mod template;
pub mod types;
pub mod validate;
