# Change Log

## [Unreleased]

## [0.7.8]

* Fix unconditional failure at `RiReferenceStr::set_fragment()`

### Fixed
* Fix unconditional failure at `RiReferenceStr::set_fragment()`

## [0.7.7]

* Fix URI template expansion with `template::DynamicContext` to call
  the methods `on_expansion_start` and `on_expansion_end`.

### Fixed
* Fix URI template expansion with `template::DynamicContext` to call
  the methods `on_expansion_start` and `on_expansion_end`.

## [0.7.6]

* Add `fragment_str()` methods that returns a fragment in a raw string slice.

### Added
* Add `fragment_str()` methods that returns a fragment in a raw string slice.
    + List of added methods:
        + `RiStr::fragment_str()`
        + `RiReferenceStr::fragment_str()`
        + `RiRelativeStr::fragment_str()`

## [0.7.5]

* Fix unsoundness of `template::UriTemplateStr`

### Fixed
* Fix unsoundness of `template::UriTemplateStr`
    * The type should have `#[repr(transparent)]` to compile safely but did not.
    * Any creations and uses of the value are undefined behavior without the
      fix, while the current version of the Rust compiler seems to happen to
      generate the expected binary (without any guarantee).

## [0.7.4]

* Fix calculation of template expansion error location.
    + Currently this just appears in an error message but not exposed through
      any other public API.
* Add an iterator of variables that appears in a URI template.
* Support URI template expansion with a mutable context.

### Added
* Add an iterator of variables that appears in a URI template.
    + List of added items:
        - `template::UriTemplateStr::variables()` method
        - `template::UriTemplateVariables<'_>` iterator type
* Support URI template expansion with a mutable context.
    + Add methods `template::UriTemplateStr::expand_dynamic()` and
      `template::UriTemplateStr::expand_dynamic_to_string()`.
    + Add `template::context::DynamicContext` trait for mutable context.
    + Add `template::context::Visitor::purpose()` method and
      `template::context::VisitPurpose` type to enable users to know for what
      purpose the variable is being visited.

### Fixed
* Fix calculation of template expansion error location.
    + Currently this error location info just appears in an error message (from
      `<template::Error as std::fmt::Display>::fmt`), but not exposed through
      any other public API.

## [0.7.3]

* Add easy conversion from an expanded template into IRI/URI string types.

### Added
* Add easy conversion from an expanded template into IRI/URI string types.
    + List of added conversions:
        - `TryFrom<template::Expanded<'_, S, C>> for types::RiAbsoluteString<S>`
        - `TryFrom<template::Expanded<'_, S, C>> for types::RiReferenceString<S>`
        - `TryFrom<template::Expanded<'_, S, C>> for types::RiRelativeString<S>`
        - `TryFrom<template::Expanded<'_, S, C>> for types::RiString<S>`

## [0.7.2]

* Fix a bug that some abnormal IRIs that have no authority and end with `/.`
  resulted in wrong normalization that generate unintentional authorities.
    + Reported at [#36](https://github.com/lo48576/iri-string/issues/36).
* Fix a bug that the normalization incorrectly omits percent-encoded triplets
  partially if they constitute invalid UTF-8 byte sequence.
    + Reported at [#36](https://github.com/lo48576/iri-string/issues/36#issuecomment-2053688909).

### Fixed
* Fix a bug that some abnormal IRIs that have no authority and end with `/.`
  resulted in wrong normalization that generate unintentional authorities.
    + Reported at [#36](https://github.com/lo48576/iri-string/issues/36).
    + IRI resolution and normalization had this bug, but only for IRIs without authority.
    + This happened when the resolution and normalization result should not contain
      an authority but the path part resulted in `//.`.
        - For example, `a:/.//.` and `a:/bar/..//.` should be normalized to `a:/.//`,
          but the actual result was `a://` due to this bug.
* Fix a bug that the normalization incorrectly omits percent-encoded triplets
  partially if they constitute invalid UTF-8 byte sequence.
    + Reported at [#36](https://github.com/lo48576/iri-string/issues/36#issuecomment-2053688909).
    + URIs and IRIs that only contains the percent-encoded triplets for valid UTF-8 byte
      sequences won't be affected.

## [0.7.1]

* Add `new_unchecked()` methods to string types.
* Move `template::VarName` type into `template::context` module and deprecate the old name.
* Add `template::context::VarName::new()` method.
* Add component getters to `resolve::FixedBaseResolver`.
* Fix some lint warnings detected by newer clippy.

### Added
* Add `new_unchecked()` methods to string types.
    + List of added methods:
        - `template::UriTemplateStr::new_unchecked()`
        - `template::UriTemplateString::new_unchecked()`
        - `types::RiAbsoluteStr::new_unchecked()`
        - `types::RiAbsoluteString::new_unchecked()`
        - `types::RiFragmentStr::new_unchecked()`
        - `types::RiFragmentString::new_unchecked()`
        - `types::RiQueryStr::new_unchecked()`
        - `types::RiQueryString::new_unchecked()`
        - `types::RiReferenceStr::new_unchecked()`
        - `types::RiReferenceString::new_unchecked()`
        - `types::RiRelativeStr::new_unchecked()`
        - `types::RiRelativeString::new_unchecked()`
        - `types::RiStr::new_unchecked()`
        - `types::RiString::new_unchecked()`
* Add `template::context::VarName::new()` method.
* Add component getters to `resolve::FixedBaseResolver`.
    + List of added methods:
        - `resolve::FixedBaseResolver::scheme_str()`
        - `resolve::FixedBaseResolver::authority_str()`
        - `resolve::FixedBaseResolver::path_str()`
        - `resolve::FixedBaseResolver::query()`
        - `resolve::FixedBaseResolver::query_str()`
        - `resolve::FixedBaseResolver::fragment_str()`

### Changed (non-breaking)
* Move `template::VarName` type into `template::context` module and deprecate the old name.
    + The old name (`template::VarName`) is still available while it is marked as deprecated.

## [0.7.0]

* Add `template` module that contains URI Template
  ([RFC 6570](https://www.rfc-editor.org/rfc/rfc6570)) processor.
* Add `PercentEncoded::{unreserve,characters}` methods.
* Remove "WHATWG" normalization.
    + Fixes the issue [#29](https://github.com/lo48576/iri-string/issues/29) and
      [#30](https://github.com/lo48576/iri-string/issues/30).
* Add normalization that preserves relative path in some special condition.

### Added
* Add `template` module that contains URI Template
  ([RFC 6570](https://www.rfc-editor.org/rfc/rfc6570)) processor.
    + The processor supports nostd environment.
* Add `PercentEncoded::{unreserve,characters}` methods.
    + List of added methods:
        - `percent_encode::PercentEncoded::characters()`
        - `percent_encode::PercentEncoded::unreserve()`
* Add normalization that preserves relative path in some special condition.
    + When the authority component is absent and the path is relative, the dot-segments
      removal is not applied to the path. This behavior is inspired by WHATWG URL
      Standard, but the implementation is not guaranteed to follow that spec.
    + List of added items:
        - `types::RiStr::normalize_but_preserve_authorityless_relative_path()`
        - `types::RiStr::is_normalized_but_authorityless_relative_path_preserved()`
        - `types::RiAbsoluteStr::normalize_but_preserve_authorityless_relative_path()`
        - `types::RiAbsoluteStr::is_normalized_but_authorityless_relative_path_preserved()`
        - `normalize::Normalized::and_normalize_but_preserve_authorityless_relative_path()`
        - `normalize::Normalized::enable_normalization_preserving_authorityless_relative_path()`
    + Note that this normalization algorithm is not compatible with RFC 3986
      algorithm for some inputs.

### Changed (breaking)
* Remove non-compliant "WHATWG" normalization.
    + Fixes the issue [#29](https://github.com/lo48576/iri-string/issues/29) and
      [#30](https://github.com/lo48576/iri-string/issues/30).
    + Previous implementations of normalization is described as "defined in WHATWG spec",
      but they were not compliant to the spec. Specifically, when the authority
      component is absent and the path is relative, WHATWG spec requires the path
      to be treated as "opaque", but the old implementation applied dot-segments
      removal to the path.
    + List of removed items:
        - `types::RiStr::is_normalized_whatwg()`
        - `types::RiAbsoluteStr::is_normalized_whatwg()`

## [0.6.0]

* Bump MSRV to 1.60.0.
* Remove `memchr-std`, `serde-alloc`, and `serde-std` features.
    + Now `alloc` and/or `std` features for additional dependencies are
      automatically enabled when all of dependent featuers are enabled.
        - See [Announcing Rust 1.60.0 | Rust Blog](https://blog.rust-lang.org/2022/04/07/Rust-1.60.0.html#new-syntax-for-cargo-features).
* Support escaping username and password by `percent_encode::PercentEncode`.
* Add `format` module that contains utilities for types with `Display` trait impl.
    + Add `format::ToStringFallible` trait.
    + Add `format::ToDedicatedString` trait.
    + Add `format::write_to_slice` function and `format::CapacityOverflow` type.
    + Add `format::try_append_to_string` function.
* Remove `task` module and `task::ProcessAndWrite` trait.
    + Remove `task` module.
    + Remove `ProcessAndWrite` trait implementation from `percent_encode::PercentEncoded` type.
    + Remove `ProcessAndWrite` trait implementation from `convert::MappedToUri` type.
* Remove "task" types.
    + Remove `normalize::NormalizationTask` type.
    + Remove `normalize::NormalizationTask` type.
        - Use `normalize::Normalized` instead.
* Change return types of some functions from task types or string types to `Display`-able types.
    + Change return types of `{BorrowedIri}::encode_to_uri` to
      `convert::MappedToUri<'_, Self>`.
    + Change return type of `resolve::FixedBaseResolver::resolve()` method to `noramlize::Normalized`.
    + Change return type of `{BorrowedIri}::normalize()` method to `normalize::Normalized`.
    + Change return type of `{BorrowedIri}::resolve_against()` method to `normalize::Normalized`.
    + Remove `BufferError` type.
* Change API of IRI-to-URI conversion.
    + Rename `{OwnedIri}::encode_to_uri` to `{OwnedIri}::encode_to_uri_inline`.
    + Add `{OwnedIri}::try_encode_to_uri_inline` method.
    + Add `{OwnedIri}::try_encode_into_uri` method.
* Make the methods impl of `convert::MappedToUri<'_, T>` generic over the spec.
* Revome functions under `resolve` module.
* Add `normalize::Normalized` type.
* Remove some methods of `resolve::FixedBaseResolver`.
* Rename `{BorrowedIri}::is_normalized()` methods to `{BorrowedIri}::is_normalized_rfc3986()`.
* Remove some methods of borrowed IRI string types.
* Support password masking.
    + Add `mask_password` module.
    + Add `{BorrowedIri}::mask_password` method.
    + Add `{OwnedIri}::remove_password_inline` and `{OwnedIri}::remove_nonempty_password_inline()`
      methods.
* Remove deprecated `percent_encoding` module and aliases defined in it.
* Unify normalization of `build::Builder`.
    + Add `build::Builder::normalize()` method.
    + Add `build::Built::ensure_rfc3986_normalizable()` method.
    + Change return type of `build::Builder::build()`.
    + Remove `build::Builder::normalize_rfc3986()` and
      `build::Builder::normalize_whatwg()` methods.
    + Remove `build::Error` type.
* Stop accepting user part as `Option<&str>` type for `build::Builder::userinfo`
* Reject user with colon characters on IRI build.
* Allow builders to normalize `path` component of relative IRIs if safely possible.

### Added
* Support escaping username and password by `percent_encode::PercentEncode`.
    + List of added functions:
        `percen_encode::PercentEncode::from_user()`
        `percen_encode::PercentEncode::from_password()`
* Add `format::ToStringFallible` trait.
    + This trait allows users to convert `Display`-able values into `String`,
      but without panicking on OOM.
    + List of types that implements this trait:
        - `build::Built<'_, RiReferenceStr<S>>`
        - `build::Built<'_, RiStr<S>>`
        - `build::Built<'_, RiAbsoluteStr<S>>`
        - `build::Built<'_, RiRelativeStr<S>>`
* Add `format::ToDedicatedString` trait.
    + This trait allows users to convert `Display`-able values into owned
      dedicated IRI string types, with or without panicking on OOM.
    + List of added implementations:
        - `build::Built<'_, RiReferenceStr<S>>` (Target = `RiReferenceString<S>`)
        - `build::Built<'_, RiStr<S>>` (Target = `RiString<S>`)
        - `build::Built<'_, RiAbsoluteStr<S>>` (Target = `RiAbsoluteString<S>`)
        - `build::Built<'_, RiRelativeStr<S>>` (Target = `RiRelativeString<S>`)
        - `convert::MappedToUri<'_, RiReferenceStr<S>>` (Target = `RiReferenceString<S>`)
        - `convert::MappedToUri<'_, RiStr<S>>` (Target = `RiString<S>`)
        - `convert::MappedToUri<'_, RiAbsoluteStr<S>>` (Target = `RiAbsoluteString<S>`)
        - `convert::MappedToUri<'_, RiRelativeStr<S>>` (Target = `RiRelativeString<S>`)
        - `convert::MappedToUri<'_, RiQueryStr<S>>` (Target = `RiQueryString<S>`)
        - `convert::MappedToUri<'_, RiFragmentStr<S>>` (Target = `RiFragmentString<S>`)
* Add `format::write_to_slice` function and `format::CapacityOverflow` type.
* Add `format::try_append_to_string` function.
* Add `{OwnedIri}::try_encode_to_uri_inline` method.
* Add `{OwnedIri}::try_encode_into_uri` method.
* Add `normalize::Normalized` type.
    + This replaces `normalize::NormalizationTask` type in previous versions.
* Add `mask_password` module.
    + Items in this module let users hide or replace password to keep sensitive
      information secret.
    + List of added items:
        - `mask_password::PasswordMasked` type
        - `mask_password::PasswordReplaced` type
* Add `{BorrowedIri}::mask_password` method.
    + List of added methods:
        - `types::RiReferenceStr::mask_password()`
        - `types::RiStr::mask_password()`
        - `types::RiAbsoluteStr::mask_password()`
        - `types::RiRelativeStr::mask_password()`
* Add `{OwnedIri}::remove_password_inline` and `{OwnedIri}::remove_nonempty_password_inline()`
  methods.
    + List of added methods:
        - `types::RiReferenceString::remove_password_inline()`
        - `types::RiReferenceString::remove_nonempty_password_inline()`
        - `types::RiString::remove_password_inline()`
        - `types::RiString::remove_nonempty_password_inline()`
        - `types::RiAbsoluteString::remove_password_inline()`
        - `types::RiAbsoluteString::remove_nonempty_password_inline()`
        - `types::RiRelativeString::remove_password_inline()`
        - `types::RiRelativeString::remove_nonempty_password_inline()`
* Add `build::Builder::normalize()` method.
* Add `build::Built::ensure_rfc3986_normalizable()` method.

### Changed (breaking)
* Bump MSRV to 1.60.0.
* Remove `memchr-std`, `serde-alloc`, and `serde-std` features.
    + Now `alloc` and/or `std` features for additional dependencies are
      automatically enabled when all of dependent featuers are enabled.
        - See [Announcing Rust 1.60.0 | Rust Blog](https://blog.rust-lang.org/2022/04/07/Rust-1.60.0.html#new-syntax-for-cargo-features).
* Remove `ProcessAndWrite` trait implementation from `percent_encode::PercentEncoded` type.
* Remove `ProcessAndWrite` trait implementation from `convert::MappedToUri` type.
* Change return types of `{BorrowedIri}::encode_to_uri` to
  `convert::MappedToUri<'_, Self>`.
    + The code `borrowed.encode_to_uri()` in older versions should be rewritten
      to `borrowed.encode_to_uri().to_dedicated_string()`.
* Rename `{OwnedIri}::encode_to_uri` to `{OwnedIri}::encode_to_uri_inline`.
* Remove `normalize::NormalizationTask` type.
    + Use `normalized::Normalized` type instead.
* Revome functions under `resolve` module.
    + List of removed functions:
        - `resolve::resolve()`
        - `resolve::resolve_normalize()`
        - `resolve::resolve_whatwg()`
        - `resolve::resolve_normalize_whatwg()`
        - `resolve::try_resolve()`
        - `resolve::try_resolve_normalize()`
        - `resolve::try_resolve_whatwg()`
        - `resolve::try_resolve_normalize_whatwg()`
* Remove `normalize::NormalizationTask` type.
    + Use `normalize::Normalized` instead.
* Remove some methods of `resolve::FixedBaseResolver`.
    + List fo removed methods:
        - `resolve::FixedBaseResolver::try_resolve()`
        - `resolve::FixedBaseResolver::try_resolve_normalize()`
        - `resolve::FixedBaseResolver::resolve_normalize()`
        - `resolve::FixedBaseResolver::create_task()`
        - `resolve::FixedBaseResolver::create_normalizing_task()`
* Change return type of `resolve::FixedBaseResolver::resolve()` to `noramlize::Normalized`.
* Rename `{BorrowedIri}::is_normalized` methods to `{BorrowedIri}::is_normalized_rfc3986`.
    + List of renamed methods:
        - `types::RiStr::is_normalized` to `types::RiStr::is_normalized_rfc3986`
        - `types::RiAbsoluteStr::is_normalized` to `types::RiAbsoluteStr::is_normalized_rfc3986`
* Change return type of `{BorrowedIri}::normalize()` method to `normalize::Normalized`.
    + List of affected methods:
        - `types::RiStr::normalize()`
        - `types::RiAbsoluteStr::normalize()`
* Remove some methods of borrowed IRI string types.
    + List of removed methods:
        - `types::RiReferenceStr::try_resolve_against()`
        - `types::RiReferenceStr::try_resolve_whatwg_against()`
        - `types::RiReferenceStr::resolve_whatwg_against()`
        - `types::RiReferenceStr::try_resolve_normalize_against()`
        - `types::RiReferenceStr::resolve_normalize_against()`
        - `types::RiReferenceStr::try_resolve_normalize_whatwg_against()`
        - `types::RiReferenceStr::resolve_normalize_whatwg_against()`
        - `types::RiStr::try_normalize()`
        - `types::RiStr::try_normalize_whatwg()`
        - `types::RiStr::normalize_whatwg()`
        - `types::RiAbsoluteStr::try_normalize()`
        - `types::RiAbsoluteStr::try_normalize_whatwg()`
        - `types::RiAbsoluteStr::normalize_whatwg()`
        - `types::RiRelativeStr::try_resolve_against()`
        - `types::RiRelativeStr::try_resolve_whatwg_against()`
        - `types::RiRelativeStr::resolve_whatwg_against()`
        - `types::RiRelativeStr::try_resolve_normalize_against()`
        - `types::RiRelativeStr::resolve_normalize_against()`
        - `types::RiRelativeStr::try_resolve_normalize_whatwg_against()`
        - `types::RiRelativeStr::resolve_normalize_whatwg_against()`
* Change return type of `{BorrowedIri}::resolve()` method to `normalize::Normalized`.
    + List of affected methods:
        - `types::RiReferenceStr::resolve_against()`
        - `types::RiRelativeStr::resolve_against()`
* Remove `BufferError` type.
* Remove `task` module.
    + List of removed items:
        - `task::Error` type
        - `task::ProcessAndWrite` trait
* Remove deprecated `percent_encoding` module and aliases defined in it.
    + List of removed items:
        - `percent_encoding` module.
        - `percent_encoding::PercentEncoded` type alias.
        - `percent_encoding::PercentEncodedForIri` type alias.
        - `percent_encoding::PercentEncodedForUri` type alias.
* Change return type of `build::Builder::build()`.
    + Now it returns `Result<(), validate::Error>` instead of
      `Result<(), build::Error>`.
* Remove `build::Builder::normalize_rfc3986()` and
  `build::Builder::normalize_whatwg()` methods.
    + Use `build::Builder::normalize()` and
      `build::Builder::ensure_rfc3986_normalizable()` instead.
* Remove `build::Error` type.
* Stop accepting user part as `Option<&str>` type for `build::Builder::userinfo`
    + Now user part should be non-optional (but possibly empty) `&str`.

### Changed (non-breaking)
* Make methods of `convert::MappedToUri<'_, T>` generic over the spec.
    + Now methods of `convert::MappedToUri<'_, T>` can be called for
      `{BorrowedIri}<S> where S: Spec`.
* Reject user with colon characters on IRI build.
    + Now `build::Builder::build()` fails when `user` part contains a colon (`:`).
* Allow builders to normalize `path` component of relative IRIs if safely possible.

## [0.5.6]

* Fix normalization bug.
    + Previously, trailing colon of an authority (with empty port) was not
      stripped. Now this is fixed.
* Add `ensure_rfc3986_normalizable()` methods to absolute IRI string types.
* Add IRI builder in `build` module.
* Deprecate `percent_encoding` module in favor of the new name `percent_encode`.

### Added
* Add `ensure_rfc3986_normalizable()` methods to absolute IRI string types.
    + List of added functions:
        - `types::RiStr::ensure_rfc3986_normalizable()`
        - `types::RiAbsoluteStr::ensure_rfc3986_normalizable()`
* Add IRI builder in `build` module.
    + `Builder` type is a builder.
    + `DisplayBulid` type is a validated build result (but not yet heap-allocates).
    + `PortBuilder` and `UserinfoBuilder` types are intermediate types to
      provide convenient generics to component setters.
    + `Error` type is a builder error.
    + `Buildable` trait indicates the syntax corresponding to the string types
      (such as `IriStr` or `UriReferenceStr`) can be constructed by the builder.

### Fixed
* Fix normalization bug.
    + Previously, trailing colon of an authority (with empty port) was not
      stripped. Now this is fixed.

### Changed (non-breaking)
* Deprecate `percent_encoding` module in favor of the new name `percent_encode`.
    + Previously exported items are still provided from `percent_encoding`
      module to keep backward compatibility.

## [0.5.5]

* Add `RiQueryStr` and `RiQueryString` types for query.
* Add functions with `try_` prefix are introduced for normalization and
  IRI resolution, and deprecate non-`try` versions.
* Add encoder types for percent-encoding in `percent_encoding` module.

### Added
* Add `RiQueryStr` and `RiQueryString` types for query.
* Add functions with `try_` prefix are introduced for normalization and resolution.
    + List of added functions:
        - `types::RiStr::try_normalize()`
        - `types::RiStr::try_normalize_whatwg()`
        - `types::RiAbsoluteStr::try_normalize()`
        - `types::RiAbsoluteStr::try_normalize_whatwg()`
        - `resolve::try_resolve()`
        - `resolve::try_resolve_whatwg()`
        - `resolve::try_resolve_normalize()`
        - `resolve::try_resolve_normalize_whatwg()`
        - `types::RiReferenceStr::try_resolve_against()`
        - `types::RiReferenceStr::try_resolve_normalize_against()`
        - `types::RiReferenceStr::try_resolve_whatwg_against()`
        - `types::RiReferenceStr::try_resolve_normalize_whatwg_against()`
        - `types::RiRelativeStr::try_resolve_against()`
        - `types::RiRelativeStr::try_resolve_normalize_against()`
        - `types::RiRelativeStr::try_resolve_whatwg_against()`
        - `types::RiRelativeStr::try_resolve_normalize_whatwg_against()`
* Add encoder types for percent-encoding in `percent_encoding` module.
    + List of added types:
        - `percent_encoding::PercentEncoded`
        - `percent_encoding::PercentEncodedForIri` (type alias)
        - `percent_encoding::PercentEncodedForUri` (type alias)

### Changed (non-breaking)
* Deprecate non-`try` function names for normalization and resolution.
    + List of deprecated functions:
        - `types::RiStr::normalize()`
        - `types::RiStr::normalize_whatwg()`
        - `types::RiAbsoluteStr::normalize()`
        - `types::RiAbsoluteStr::normalize_whatwg()`
        - `resolve::resolve()`
        - `resolve::resolve_whatwg()`
        - `resolve::resolve_normalize()`
        - `resolve::resolve_normalize_whatwg()`
        - `types::RiReferenceStr::resolve_against()`
        - `types::RiReferenceStr::resolve_normalize_against()`
        - `types::RiReferenceStr::resolve_whatwg_against()`
        - `types::RiReferenceStr::resolve_normalize_whatwg_against()`
        - `types::RiRiAblosuteStrStr::resolve_against()`
        - `types::RiRiAblosuteStrStr::resolve_normalize_against()`
        - `types::RiRiAblosuteStrStr::resolve_whatwg_against()`
        - `types::RiRiAblosuteStrStr::resolve_normalize_whatwg_against()`
    + Use functions with `try_` prefix instead.

## [0.5.4]

* Implement IRI resolution and normalization that uses serialization algorithm
  [described in WHATWG URL Standard](https://url.spec.whatwg.org/#url-serializing).

### Added
* Implement IRI resolution and normalization that uses serialization algorithm
  [described in WHATWG URL Standard](https://url.spec.whatwg.org/#url-serializing).
    + They won't fail even when the input or result is abnormal (but of course
      they may still fail on memory shortage).
    + The difference between RFC 3986/3987 versions and WHATWG versions is,
      handling of absent host and path starting with `//`. The RFC versions fail
      since `scheme://not-a-host` is invalid, but WHATWG versions serializes the
      result as `scheme:/.//not-a-host`.
    + List of added functions:
        - `resolve::resolve_whatwg()`
        - `resolve::resolve_normalize_whatwg()`
        - `normalize::NormalizationTask::enable_normalization()`
        - `normalize::NormalizationTask::enable_whatwg_serialization()`
        - `types::RiStr::is_normalized_whatwg()`
        - `types::RiStr::normalize_whatwg()`
        - `types::RiAbsoluteStr::is_normalized_whatwg()`
        - `types::RiAbsoluteStr::normalize_whatwg()`
        - `types::RiReferenceStr::resolve_normalize_whatwg_against()`
        - `types::RiReferenceStr::resolve_whatwg_against()`
        - `types::RiRelativeStr::resolve_normalize_whatwg_against()`
        - `types::RiRelativeStr::resolve_whatwg_against()`

## [0.5.3]

* Decode percent-encoded unreserved characters on normalizaiton.
* Add `is_normalized` method to absolute URI/IRI types.
* Implement more conversion traits from string types to `Cow`, `Box`, `Rc`, and `Arc`.
* Improve documents.

### Added
* Add `is_normalized` method to absolute URI/IRI types.
    + They don't heap-allocate.
* Implement more conversion traits from string types to `Cow`, `Box`, `Rc`, and `Arc`.
    + List of added conversions:
        - `From<&'a $slice> for Cow<'a, $slice>`
        - `From<&'_ $slice> for Box<$slice>`
        - `From<&'_ $slice> for Rc<$slice>`
        - `From<&'_ $slice> for Arc<$slice>`
        - `From<$owned> for Cow<'_, $owned>`
        - `From<$owned> for Box<$owned>`

### Fixed
* Decode percent-encoded unreserved characters on normalizaiton.
    + Previous implementation incorrectly leave unreserved percent-encoded
      characters as is, but now this is fixed.


## [0.5.2]

* Fix IPvFuture literal parsing again (<https://github.com/lo48576/iri-string/issues/17>).

### Fixed
* Fix IPvFuture literal parsing again (<https://github.com/lo48576/iri-string/issues/17>).

## [0.5.1]

* Add `FixedBaseResolver::base()` method.
* Fix IP literal parsing and decomposition (<https://github.com/lo48576/iri-string/issues/17>).

### Added
* Add `FixedBaseResolver::base()` method.

### Fixed
* Fix IP literal parsing and decomposition (<https://github.com/lo48576/iri-string/issues/17>).

## [0.5.0]

This entry describes the changes since the previous stable release (v0.4.1).

* Bump MSRV to 1.58.0.
* Add more conversions from/to IRI string types.
    + Implement `TryFrom<&[u8]>` for the IRI string types.
    + Implement `From<{owned URI}>` for the owned IRI string types.
    + Add `as_slice` method to the owned string types.
    + Add `convert::MappedToUri` type.
    + Add `encode_to_uri()` method for the IRI string types.
    + Add `encode_into_uri()` method for the owned IRI string types.
    + Add `as_uri()` method for the borrowed IRI string types.
    + Add `try_into_uri()` method for the owned IRI string types.
* Add `capacity()` method to the owned string types.
* Add components getters for borrowed string types.
* Add IRI normalization API and related types.
* Add normalizing variations for IRI resolution.
* Support nostd for IRI resolution.
* Change IRI resolution API incompatibly.
    + Change number and types of parameters.
    + Change return types.
* Let IRI resolution recognize percent-encoded period during normalization.
* Drop internal dependency to `nom` crate.
* Permit `serde`+`{alloc,std}` without `serde-{alloc,std}`.
* Update examples.
    + Improve `parse` example to show more information.
    + Add `normalize` example.
* Travis CI is no longer used.
    + Checks should be run manually. See README for detail.

### Added
* Add more conversions from/to IRI string types.
    + Implement `TryFrom<&[u8]>` for the IRI string types.
    + Implement `From<{owned URI}>` for the owned IRI string types.
    + Add `as_slice` method to the owned string types.
    + Add `convert::MappedToUri` type.
    + Add `encode_to_uri()` method for the IRI string types.
    + Add `encode_into_uri()` method for the owned IRI string types.
    + Add `as_uri()` method for the borrowed IRI string types.
    + Add `try_into_uri()` method for the owned IRI string types.
* Add `capacity()` method to the owned string types.
* Add components getters for borrowed string types.
    + Add getters for major components of IRIs/URIs:
      `scheme`, `authority`, `path`, and `query`.
    + Add types and getters for subcomponents of `authority`:
      `userinfo`, `host`, and `port`.
        - `components::AuthorityComponents` type and `authority_components` method.
* Add IRI normalization API and related types.
    + Add `{RiStr, RiAbsoluteStr}::normalize()` methods.
    + Add `normalize::NormalizationTask`.
* Add normalizing variations for IRI resolution.
    + `resolve_normalize` for `resolve`.
    + `resolve_normalize_against` for `resolve_against`.
* Support nostd for IRI resolution.
    + Add `resolve::FixedBaseResolver` to get `normalize::NormalizationTask`.
    + Users can write the resolution/normalization result to user-provided buffer by `NormalizationTask`.
* Update examples.
    + Add `normalize` example.

### Changed (breaking)
* Bump MSRV to 1.58.0.
    + Rust 1.58.0 is released at 2022-01-13.
* Change IRI resolution API incompatibly.
    + Remove `is_strict: bool` parameter from `resolve::resolve()`.
    + Make IRI resolution fallible.
        - Now IRI resolution returns `Result`.
        - For details about possible resolution/normalization failure, see the
          documentation for `normalize` module and the issue [How `/..//bar`
          should be resolved aganst `scheme:`?
          (#8)](https://github.com/lo48576/iri-string/issues/8).

### Changed (non-breaking)
* Let IRI resolution recognize percent-encoded period during normalization.
    + For example, `/%2e%2e/` in the path are now recognized as `/../`, and
      handled specially as "parent directory".
* Drop internal dependency to `nom` crate.
    + Parsers are rewritten manually, and are now much faster than before.
* Permit `serde`+`{alloc,std}` without `serde-{alloc,std}`.
    + Previously, compilation error are intentionally caused when both `serde`
      and `{alloc,std}` are enabled but corresponding `serde-{alloc,std}` is not.
    + Note that you still need to enable `serde-alloc` or `serde-std` to use
      serde support for owned string types.
        - This change only intends to support the cases when flags are
          independently enabled from different indirect dependencies.
* Update examples.
    + Improve `parse` example to show more information.

## ([0.5.0] from [0.5.0-rc.0])

* Travis CI is no longer used.
    + Checks should be run manually. See README for detail.

## [0.5.0-rc.0]

No more API changes are planned until v0.5.0.

* Add more conversions from IRI to URI string types.
    + Add `as_uri()` method for the borrowed IRI string types.
    + Add `try_into_uri()` method for the owned IRI string types.
* Update examples.
    + Improve `parse` example to show more information.
    + Add `normalize` example.

### Added
* Add more conversions from IRI to URI string types.
    + Add `as_uri()` method for the borrowed IRI string types.
    + Add `try_into_uri()` method for the owned IRI string types.
* Update examples.
    + Add `normalize` example.

### Changed (non-breaking)
* Update examples.
    + Improve `parse` example to show more information.

## [0.5.0-beta.4]

* Add more conversions from/to IRI string types.
    + Implement `From<{owned URI}>` for the owned IRI string types.
    + Add `as_slice` method to the owned string types.
    + Add `convert::MappedToUri` type.
    + Add `encode_to_uri()` method for the IRI string types.
    + Add `encode_into_uri()` method for the owned IRI string types.
* Refine task API
    + Move some methods of `normalize::NormalizationTask` into newly added
      `task::ProcessAndWrite` trait.
        - `allocate_and_write`, `write_to_byte_slice`, `append_to_std_string`,
          and `try_append_to_std_string` is moved.
    + Change type parameter of `NormalizationTask` from a spec into a string slice type.
    + Change error type for `NormalizationTask`.
    + Remove `normalize::create_task()` function.

### Added
* Add more conversions from/to IRI string types.
    + Implement `From<{owned URI}>` for the owned IRI string types.
    + Add `as_slice` method to the owned string types.
    + Add `convert::MappedToUri` type.
    + Add `encode_to_uri()` method for the IRI string types.
    + Add `encode_into_uri()` method for the owned IRI string types.

### Changed (breaking)
* Move some methods of `normalize::NormalizationTask` into newly added
  `task::ProcessAndWrite` trait.
    + `allocate_and_write`, `write_to_byte_slice`, `append_to_std_string`,
      and `try_append_to_std_string` is moved.
* Change type parameter of `NormalizationTask` from a spec into a string slice type.
    + Now `NormalizationTask<S>` should be changed to `NormalizationTask<RiStr<S>>`
      or `NormalizationTask<RiAbsoluteStr<S>>`.
    + This enables the task to return more appropriate type. For example,
      returning `&RiAbsoluteStr<S>` rather than `&RiStr<S>` when the input IRI
      type is `RiAbsoluteStr<S>`.
* Change error type for `NormalizationTask`.
    + Now buffer error and processing error is split to different types.
* Remove `normalize::create_task()` function.

## [0.5.0-beta.3]

* Add `normalize` module, and unify it with IRI resolution.
    + Move `resolve::{Error, ErrorKind}` to `normalize` module.
    + Move and rename `resolve::ResolutionTask` to `normalize::NormalizationTask`.
    + Add `normalize::create_task` function.
    + Add `{RiStr, RiAbsoluteStr}::normalize()` methods.
* Add normalizing variations for IRI resolution.

### Added
* Add `normalize` module.
    + Add `normalize::create_task()` function.
    + Add `{RiStr, RiAbsoluteStr}::normalize()` methods.
* Add normalizing variations for IRI resolution.
    + `resolve_normalize` for `resolve`.
    + `resolve_normalize_against` for `resolve_against`.

### Changed (breaking)
* Move `resolve::{Error, ErrorKind}` to `normalize` module.
* Move and rename `resolve::ResolutionTask` to `normalize::NormalizationTask`.
    + Now `resolve::FixedBaseResolver::create_task()` returns `NormalizationTask`.

## [0.5.0-beta.2]

* Fix a bug that `serde-std` feature did not enable serde support for owned types.

### Fixed
* Fix a bug that `serde-std` feature did not enable serde support for owned types.
    + Now `serde-std` enables `alloc` features automatically.

## [0.5.0-beta.1]

* Add getters for major components of IRIs/URIs: `scheme`, `authority`, `path`, and `query`.
* Add types and getters for subcomponents of `authority`: `userinfo`, `host`, and `port`.
    + `components::AuthorityComponents` type and `authority_components` method.
* Fix a bug that `serde-std` feature did not enable serde support for owned types.

### Added
* Add getters for major components of IRIs/URIs: `scheme`, `authority`, `path`, and `query`.
    + Method names are `scheme_str`, `authority_str`, `path_str`, and `query_str`, respectively.
    + Getters for `fragment` component is already provided.
* Add getter for subcomponents of `authority`: `userinfo`, `host`, and `port`.
    + `components::AuthorityComponents` type and `authority_components` method.

### Fixed
* Fix a bug that `serde-std` feature did not enable serde support for owned types.
    + Now `serde-std` enables `alloc` features automatically.

## [0.5.0-beta.0]

* Bump MSRV to 1.58.0.
* Add conversion from a byte slice (`&[u8]`) into IRI string types.
* Add `capacity` method to allocated string types.
* Remove `is_strict: bool` parameter from `resolve::resolve()`.
* Add `resolve::FixedBaseResolver`, `resolve::ResolutionTask`, and `resolve::Error` types.
    + Some methods for IRI resolution are now available even when `alloc` feature is disabled.
    + See [IRI resolution using user-provided buffers (#6)](https://github.com/lo48576/iri-string/issues/6).
* Make IRI resolution fallible.
    + Now `resolve()` and its family returns `Result<_, resolve::Error>`.
    + See [How `/..//bar` should be resolved aganst `scheme:`? (#8)](https://github.com/lo48576/iri-string/issues/8).
* Make IRI resolution recognize percent-encoded period.
    + Now `%2E` and `%2e` in path segment is handled as a plain period `.`.
    + See [Recognize percent-encoded periods (`%2E`) during IRI resolution (#9)](https://github.com/lo48576/iri-string/issues/9)
* Make parsers faster.
    + See [Make the parsers faster (#7)](https://github.com/lo48576/iri-string/issues/7)
* Drop internal dependency to `nom`.
* Stop emitting compilation error when both `serde` and `std`/`alloc` are enabled
  without corresponding `serde-{std,alloc}` features.

### Added
* Add conversion from a byte slice (`&[u8]`) into IRI string types.
* Add `capacity` method to allocated string types.
    + `shrink_to_fit()` and `len()` already exists, so this would be useful to determine
      when to do `shrink_to_fit`.
* Add `resolve::FixedBaseResolver`, `resolve::ResolutionTask`, and `resolve::Error` types.
    + They provide more efficient and controllable IRI resolution.
    + Some methods for IRI resolution are now available even when `alloc` feature is disabled.

### Changed (breaking)
* Bump MSRV to 1.58.0.
    + Rust 1.58.0 is released at 2022-01-13.
* Remove `is_strict: bool` parameter from `resolve::resolve()`.
    + The IRI parsers provided by this crate is "strict", so resolution
      algorithm should use an algorithm for the strict parser.
* Make IRI resolution fallible.
    + Now `resolve()` and its family returns `Result<_, resolve::Error>`.
    + For the reasons behind, see crate-level documentation.
    + See [How `/..//bar` should be resolved aganst `scheme:`? (#8)](https://github.com/lo48576/iri-string/issues/8).
* Make IRI resolution recognize percent-encoded period.
    + Now `%2E` and `%2e` in path segment is handled as a plain period `.`.
    + Period is `unreserved` character, and can be escaped at any time
      (see [RFC 3986 section 2.4](https://datatracker.ietf.org/doc/html/rfc3986#section-2.4).
      This means that `%2E` and `%2e` in the path can be normalized to `.` before IRI resolution,
      and thus they should also be handled specially during `remove_dot_segments` algorithm.
    + See [Recognize percent-encoded periods (`%2E`) during IRI resolution (#9)](https://github.com/lo48576/iri-string/issues/9)

### Changed (non-breaking)
* Make parsers faster.
    + Parsers are rewritten, and they became very fast!
    + Almost all usages are affected: type conversions, validations, and IRI resolutions.
    + See [Make the parsers faster (#7)](https://github.com/lo48576/iri-string/issues/7)
* Drop internal dependency to `nom`.
    + Parsers are rewritten without `nom`.
* Stop emitting compilation error when both `serde` and `std`/`alloc` are enabled
  without corresponding `serde-{std,alloc}` features.
    + `serde` and `std`/`alloc` might be enabled independently from the different
      indirect dependencies, so this situation should not be compilation error.

## [0.4.1]

* Bump internal dependency.
    + `nom` from v6 to v7.

### Changed (non-breaking)
* Bump internal dependency.
    + `nom` from v6 to v7.

## [0.4.0]

* MSRV is bumped to 1.48.0.
* Internal dependencies are bumped.
    + `nom` crate is bumped to 6.
* `serde::{Serialize, Deserialize}` is now implemented only for types with valid spec types.
* Feature flags are refactored.

### Changed (breaking)
* MSRV is bumped to 1.48.0.
    + Rust 1.48.0 is released at 2020-11-19.
* `serde::{Serialize, Deserialize}` is now implemented only for types with valid spec types.
    + Strictly this is a breaking change, but this only forbids the meaningless trait impls,
      so no real world use cases won't be affected by this change.
* Feature flags are refactored.
    + `serde-alloc` and `serde-std` flags are added to control serde's alloc and std support.
    + Unintended dependency from `std` use flag to `serde` crate is now fixed.
      Users who want to enable `serde` and `std` at the same time should also enable `serde-std`
      feature. Same applies for `serde` and `alloc` pair.

## [0.3.0]

**This release contains huge changes, and CHANGELOG may be incomplete.
Beleive rustdoc rather than this CHANGELOG.**

* Minimum supported Rust version is now 1.41 or above.
* Make IRI string types polymorphic, and rename some types.
    + Now IRI types and URI types can share the same codebase.
    + This makes it easy for users to implement functions for both IRI types and URI types.
* Add URI types.
* Remove `Deref` impls for IRI string types.
* Remove depraceted items.
* Add and change methods for IRI string types.
* `resolve::resolve_iri` is now (more) polymorphic, and renamed to `resolve::resolve`.
* Update some internal dependencies.
    + This has no effect for usual users, and this does not introduce any API changes.
    + By this change, the crate now successfully compiles with minimal dependency versions.
* Support `no_std` environment.
    + `std` and `alloc` feature flags are added.
    + `std` feature is enabled by default (and `std` enables `alloc` automatically).

### Fixes
* Update some internal dependencies to make the crate buildable with minimal dependency versions.
    + This has no effect for usual users, and this does not introduce any API changes.
    + By this change, the crate now successfully compiles with minimal dependency versions.
        - To test that, you can run
          `cargo +nightly update -Z minimal-versions && cargo test --all-features`.

### Changed (breaking)
* Make IRI string types polymorphic, and rename some types.
    + Now IRI types and URI types can share the same codebase.
    + This makes it easy for users to implement functions for both IRI types and URI types.
    + Polymorphic types are named `types::Ri{,Absolute,Fragment,Reference,Relative}Str{,ing}`.
    + Type aliases for monomorphized types are also provided, but naming convertions are the same.
      They are named `{Iri,Uri}{..}Str{,ing}`.
        - For example, there is `IriAbsoluteStr` instead of legacy `AbsoluteIriStr`.
    + `types::CreationError` is now revived.
    + `types::IriCreationError` is now removed in favor of `types::CreationError`.
* Remove depraceted items.
    + `IriReferenceStr::resolve()` is now removed.
      Use `IriReferenceStr::resolve_against()` instead.
* Remove `Deref` impls for IRI string types.
    + IRI string types should not implement `Deref`, because they are not smart pointer types.
* Change methods types.
    + `IriReferenceStr::resolve_against()` now returns `Cow<'_, IriStr>`, rather than `IriString`.
* `resolve::resolve_iri` is now polymorphic, and renamed to `resolve::resolve`.
    + Now it can be used for both IRI types and URI types.

### Changed (non-breaking)
* Support `no_std` environment.
    + `std` and `alloc` feature flags are added.
    + `std` feature is enabled by default (and `std` enables `alloc` automatically).
    + In `no_std` environment with allocator support, you can enable `alloc` feature.
* Add methods for IRI string types.
    + `len()` and `is_empty()` methods are added to all IRI string slice types.
    + `IriStr::fragment()` is added.
    + `RelativeIriStr::resolve_against()` is added.
* Add URI types.

## [0.2.3]

* Fixed a bug that URI validators wrongly accepts non-ASCII characters.
    + Now they rejects non-ASCII characters correctly.
* Fixed a bug that abnormal URIs (such as `foo://` or `foo:////`) are wrongly rejected.
    + Now they are accepted as valid IRIs.

### Fixes
* Fixed a bug that URI validators wrongly accepts non-ASCII characters
  (9b8011f54dab3c2f8da78dc2251353453317d8af).
    + Now they rejects non-ASCII characters correctly.
* Fixed a bug that abnormal URIs (such as `foo://` or `foo:////`) are wrongly rejected
  (7a40f4b72964d498970a356368dc320917d88e43).
    + Now they are accepted as valid IRIs.
    + Documents are added to explain why they are valid.

### Improved
* More tests are added to ensure invalid URIs/IRIs are rejected as expected
  (9b8011f54dab3c2f8da78dc2251353453317d8af).

## [0.2.2]

* `IriReferenceStr::resolve()` is renamed to `resolve_against()`.
    + The old name will be kept until the next minor version bump to keep compatibility.

### Changed (non-breaking)
* `IriReferenceStr::resolve()` is renamed to `resolve_against()`
  (4d64ee9884713644b69b8f227f32637d877a9d5f).
    + `resolve()` was an ambiguous name, and people cannot know which `foo.resolve(bar)` means:
      "resolve foo against bar" or "foo resolves bar".
    + The new name `resolve_against()` is more clear. `foo.resolve_against(bar)` can be natuarally
      interpreted as "resolve foo against bar".
    + The old name will be kept until the next minor version bump to keep compatibility.

## [0.2.1]

* `*Str::new()` methods are added.
* `IriFragmentStr::from_prefixed()` is added.
* `types::CreationError` is renamed to `types::IriCreationError`.
    + The old name will be kept until the next minor version bump to keep compatibility.
* Reduced indirect dependencies

### Added
* `*Str::new()` methods are added (39c8f735ccf6f28aaf2f16dcdc579fb3838bb5fb).
    + Previously the string slices are created as `<&FooStr>::try_from(s)` (where `s: &str`),
      but this is redundant.
      Now `FooStr::new(s)` can be used instead of `<&FooStr>::try_from(s)` for `s: &str`.
* `IriFragmentStr::from_prefixed()` is added (34cec2f422ba8046134668bdb662f69c9db7f52c).
    + This creates `IriFragmentStr` from the given string with leading hash (`#`) character.
      For example, `IriFragmentStr::from_prefixed("#foo")` is same as `IriFragmentStr::new("foo")`.

### Changed (non-breaking)
* `types::CreationError` is renamed to `types::IriCreationError`
  (c6e930608f158281d059e632ffc6117bddf18ebc, c0e650c5e19f1775cf82960afc9610994afba66e).
    + The old name will be kept until the next minor version bump to keep compatibility.
* Disabled `lexical` feature of `nom` crate (a2d5bcd02e02e80af1c4fc8c14d768ca519ef467).
    + This reduces indirect dependencies.
* Migrate code generator from proc-macro crate to non-proc-macro one
  (363337e720a9fdfa7e17153ffc63192bd49f7cc3).
    + This reduces indirect dependencies, and may also reduce compilation time.

## [0.2.0]

* Use nom 5.0.0.
    + This is non-breaking change.

## [0.2.0-beta.1]

* Implement `Clone` and `Copy` for validation error types.
* Let an error type contain source string for conversion from owned string.
* Add `shrink_to_fit()` methods for `types::iri::*String` types.
* Add `set_fragment()` methods for `types::iri::*String` types
  (except for `AbsoluteIriString`).
* Add `as_str()` method for `types::iri::*Str` types.
* Add `types::iri::IriFragment{Str,String}` type.
* Move `fragment()` from `IriStr` to `IriReferenceStr`.

### Changed (non-breaking)
* Implement `Clone` and `Copy` for validation error types
  (`validate::{iri,uri}::Error`) (8c6af409963a).

#### Added
* Add `shrink_to_fit()` methods for `types::iri::*String` types (c8671876229f).
* Add `set_fragment()` methods for `types::iri::*String` types
  (except for `AbsoluteIriString`) (5ae09a327d93).
* Add `as_str()` method for `types::iri::*Str` types (0984140105a1).
* Add `types::iri::IriFragment{Str,String}` type (1c5e06192cf8).
    + This represents fragment part of an IRI.

### Changed (breaking)
* `types::iri::{AbsoluteIri,Iri,IriReference,RelativeIri}String::TryFrom<_>` now
  returns `types::iri::CreationError` as an error (8c6af409963a).
    + `CreationError` owns the source data so that it is not lost on conversion
      failure.
    + `CreationError::into_source()` returns the source data which cannot be
      converted into an IRI type.
    + Previously `validate::iri::Error` is used to represent error, but it does
      not own the source data.
* Move `fragment()` from `IriStr` to `IriReferenceStr` (1c5e06192cf8).
    + `v.fragment()` for `v: &IriStr` is still available thanks to `Deref`.

## [0.2.0-beta.0]

Totally rewritten.

[Unreleased]: <https://github.com/lo48576/iri-string/compare/v0.7.8...develop>
[0.7.8]: <https://github.com/lo48576/iri-string/releases/tag/v0.7.8>
[0.7.7]: <https://github.com/lo48576/iri-string/releases/tag/v0.7.7>
[0.7.6]: <https://github.com/lo48576/iri-string/releases/tag/v0.7.6>
[0.7.5]: <https://github.com/lo48576/iri-string/releases/tag/v0.7.5>
[0.7.4]: <https://github.com/lo48576/iri-string/releases/tag/v0.7.4>
[0.7.3]: <https://github.com/lo48576/iri-string/releases/tag/v0.7.3>
[0.7.2]: <https://github.com/lo48576/iri-string/releases/tag/v0.7.2>
[0.7.1]: <https://github.com/lo48576/iri-string/releases/tag/v0.7.1>
[0.7.0]: <https://github.com/lo48576/iri-string/releases/tag/v0.7.0>
[0.6.0]: <https://github.com/lo48576/iri-string/releases/tag/v0.6.0>
[0.5.6]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.6>
[0.5.5]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.5>
[0.5.4]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.4>
[0.5.3]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.3>
[0.5.2]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.2>
[0.5.1]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.1>
[0.5.0]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.0>
[0.5.0-rc.0]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.0-rc.0>
[0.5.0-beta.4]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.0-beta.4>
[0.5.0-beta.3]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.0-beta.3>
[0.5.0-beta.2]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.0-beta.2>
[0.5.0-beta.1]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.0-beta.1>
[0.5.0-beta.0]: <https://github.com/lo48576/iri-string/releases/tag/v0.5.0-beta.0>
[0.4.1]: <https://github.com/lo48576/iri-string/releases/tag/v0.4.1>
[0.4.0]: <https://github.com/lo48576/iri-string/releases/tag/v0.4.0>
[0.3.0]: <https://github.com/lo48576/iri-string/releases/tag/v0.3.0>
[0.2.3]: <https://github.com/lo48576/iri-string/releases/tag/v0.2.3>
[0.2.2]: <https://github.com/lo48576/iri-string/releases/tag/v0.2.2>
[0.2.1]: <https://github.com/lo48576/iri-string/releases/tag/v0.2.1>
[0.2.0]: <https://github.com/lo48576/iri-string/releases/tag/v0.2.0>
[0.2.0-beta.1]: <https://github.com/lo48576/iri-string/releases/tag/v0.2.0-beta.1>
[0.2.0-beta.0]: <https://github.com/lo48576/iri-string/releases/tag/v0.2.0-beta.0>
