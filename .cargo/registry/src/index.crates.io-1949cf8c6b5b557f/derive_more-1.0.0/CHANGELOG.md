# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

## 1.0.1 - Unreleased



## 1.0.0 - 2024-08-07

More than 8 years after the first commit and almost 5 years after the 0.99.0
release, `derive_more` has finally reached its 1.0.0 release. This release
contains a lot of changes (including some breaking ones) to make it easier to
use the derives and make it possible to extend them without having to breaking
backwards compatibility again. There are five major changes that I would like
to call out, but there are many more changes that are documented below:
1. There is a new `Debug` derive that can be used to easily customize `Debug`
   formatting.
2. A greatly improved `Display` derive, which allows you to do anything that
   [`thiserror`](https://github.com/dtolnay/thiserror) provides, but it works
   for any type not just errors. And by combining the `Display` derive with the
   `Error` and `From` derives, there shouldn't really be any need to use
   `thiserror` anymore (if you are missing a feature/behaviour from `thiserror`
   please report an issue).
3. Traits that can return errors now return a type that implements `Error`
   when an error occurs instead of a `&'static str`.
4. When using `use derive_more::SomeTrait` the actual trait is also imported
   not just the derive macro. This is especially useful for `Error` and
   `Display`
5. The docs are now rendered on docs.rs and are much better overall.


### Breaking changes

- The minimum supported Rust version (MSRV) is now Rust 1.75.
- Add the `std` feature which should be disabled in `no_std` environments.
- All Cargo features, except `std`, are now disabled by default. The `full`
  feature can be used to get the old behavior of supporting all possible
  derives.
- The `TryFrom`, `Add`, `Sub`, `BitAnd`, `BitOr`, `BitXor`, `Not` and `Neg`
  derives now return a dedicated error type instead of a `&'static str` on
  error.
- The `FromStr` derive now uses a dedicated `FromStrError` error type instead
  of generating unique one each time.
- The `Display` derive (and other `fmt`-like ones) now uses
  `#[display("...", (<expr>),*)]` syntax instead of
  `#[display(fmt = "...", ("<expr>"),*)]`, and `#[display(bound(<bound>))]`
  instead of `#[display(bound = "<bound>")]`. So without the double quotes
  around the expressions and bounds.
- The `Debug` and `Display` derives (and other `fmt`-like ones) now transparently
  delegate to the inner type when `#[display("...", (<expr>),*)]` attribute is
  trivially substitutable with a transparent call.
  ([#322](https://github.com/JelteF/derive_more/pull/322))
- The `DebugCustom` derive is renamed to just `Debug` (gated now under a separate
  `debug` feature), and its semantics were changed to be a superset of `std` variant
  of `Debug`.
- The `From` derive doesn't derive `From<()>` for enum variants without any
  fields anymore. This feature was removed because it was considered useless in
  practice.
- The `From` derive now uses `#[from(<types>)]` instead of `#[from(types(<types>))]`
  and ignores field type itself.
- The `Into` derive now uses `#[into(<types>)]` instead of `#[into(types(<types>))]`
  and ignores field type itself.
- The `Into` derive now generates separate impls for each field whenever the `#[into(...)]`
  attribute is applied to it. ([#291](https://github.com/JelteF/derive_more/pull/291))
- Importing a derive macro now also imports its corresponding trait.
- The `Error` derive is updated with changes to the `error_generic_member_access`
  unstable feature for nightly users. ([#200](https://github.com/JelteF/derive_more/pull/200),
  [#294](https://github.com/JelteF/derive_more/pull/294))
- The `as_mut` feature is removed, and the `AsMut` derive is now gated by the
  `as_ref` feature. ([#295](https://github.com/JelteF/derive_more/pull/295))
- A top level `#[display("...")]` attribute on an enum now requires the usage
  of `{_variant}` to include the variant instead of including it at `{}`. The
  reason is that `{}` now references the first argument to the format string,
  just like in all other format strings. ([#377](https://github.com/JelteF/derive_more/pull/377))

### Added

- Add support captured identifiers in `Display` derives. So now you can use:
  `#[display(fmt = "Prefix: {field}")]` instead of needing to use
  `#[display(fmt = "Prefix: {}", field)]`
- Add `FromStr` derive support for enums that contain variants without fields.
  If you pass the name of the variant to `from_str` it will create the matching
  variant.
- Add `#[unwrap(owned, ref, ref_mut)]` attribute for the `Unwrap` derive.
  By using them, it is possible to derive implementations for the reference types as well.
  ([#206](https://github.com/JelteF/derive_more/pull/206))
- Add `TryUnwrap` derive similar to the `Unwrap` derive. This one returns a `Result` and does not panic.
  ([#206](https://github.com/JelteF/derive_more/pull/206))
- Add support for container format in `Debug` derive with the same syntax as `Display` derives.
  ([#279](https://github.com/JelteF/derive_more/pull/279))
- `derive_more::derive` module exporting only macros, without traits.
  ([#290](https://github.com/JelteF/derive_more/pull/290))
- Add support for specifying concrete types to `AsRef`/`AsMut` derives.
  ([#298](https://github.com/JelteF/derive_more/pull/298))
- Add `TryFrom` derive for enums to convert from their discriminant.
  ([#300](https://github.com/JelteF/derive_more/pull/300))
- `#[inline]` attributes to `IsVariant` and `Debug` implementations.
  ([#334](https://github.com/JelteF/derive_more/pull/334)
- Add `#[track_caller]` to `Add`, `Mul`, `AddAssign` and `MulAssign` derives
  ([#378](https://github.com/JelteF/derive_more/pull/378)


### Changed

- The `Constructor` and `IsVariant` derives now generate `const fn` functions.
- Static methods derived by `IsVariant` are now marked `#[must_use]`.
  ([#350](https://github.com/JelteF/derive_more/pull/350))
- The `Unwrap` and `IsVariant` derives now generate doc comments.
- `#[automatically_derived]` is now emitted from all macro expansions. This
  should prevent code style linters from attempting to modify the generated
  code.
- Upgrade to `syn` 2.0.
- The `Error` derive now works in nightly `no_std` environments

### Fixed

- Use a deterministic `HashSet` in all derives, this is needed for rust analyzer
  to work correctly.
- Use `Provider` API for backtraces in `Error` derive.
- Fix `Error` derive not working with `const` generics.
- Support trait objects for source in Error, e.g.
  `Box<dyn Error + Send + 'static>`
- Fix bounds on derived `IntoIterator` impls for generic structs.
  ([#284](https://github.com/JelteF/derive_more/pull/284))
- Fix documentation of generated bounds in `Display` derive.
  ([#297](https://github.com/JelteF/derive_more/pull/297))
- Hygiene of macro expansions in presence of custom `core` crate.
  ([#327](https://github.com/JelteF/derive_more/pull/327))
- Fix documentation of generated methods in `IsVariant` derive.
- Make `{field:p}` do the expected thing in format strings for `Display` and
  `Debug`. Also document weirdness around `Pointer` formatting when using
  expressions, due to field variables being references.
  ([#381](https://github.com/JelteF/derive_more/pull/381))

## 0.99.10 - 2020-09-11

### Added

- `From` supports additional types for conversion: `#[from(types(u8, u16))]`.


## 0.99.7 - 2020-05-16

### Changed

- When specifying specific features of the crate to only enable specific
    derives, the `extra-traits` feature of  `syn` is not always enabled
    when those the specified features do not require it. This should speed up
    compile time of `syn` when this feature is not needed.

### Fixed

- Fix generic derives for `MulAssign`

## 0.99.6 - 2020-05-13

### Changed

- Make sure output of derives is deterministic, for better support in
    rust-analyzer


## 0.99.5 - 2020-03-28

### Added

- Support for deriving `Error`!!! (many thanks to @ffuugoo and @tyranron)

### Fixed

- Fix generic bounds for `Deref` and `DerefMut` with `forward`, i.e. put `Deref`
  bound on whole type, so on `where Box<T>: Deref` instead of on `T: Deref`.
  ([#107](https://github.com/JelteF/derive_more/issues/114))

- The `tests` directory is now correctly included in the crate (requested by
    Debian package maintainers)

## 0.99.4 - 2020-03-28 [YANKED]

Note: This version is yanked, because quickly after release it was found out
tests did not run in CI.

## 0.99.3 - 2020-02-19

### Fixed

- Fix generic bounds for `Deref` and `DerefMut` with no `forward`, i.e. no bounds
    are necessary. ([#107](https://github.com/JelteF/derive_more/issues/114))


## 0.99.2 - 2019-11-17

### Fixed

- Hotfix for a regression in allowed `Display` derives using `#` flag, such as
    `{:#b}` ([#107](https://github.com/JelteF/derive_more/issues/107))

## 0.99.1 - 2019-11-12

### Fixed

- Hotfix for a regression in allowed `From` derives
    ([#105](https://github.com/JelteF/derive_more/issues/105))

## 0.99.0 - 2019-11-11

This release is a huge milestone for this library.
Lot's of new derives are implemented and a ton of attributes are added for
configuration purposes.
These attributes will allow future releases to add features/options without
breaking backwards compatibility.
This is why the next release with breaking changes is planned to be 1.0.0.

### Breaking changes
- The minimum supported rust version (MSRV) is now Rust 1.36.
- When using in a Rust 2015 crate, you should add `extern crate core` to your
  code.
- `no_std` feature is removed, the library now supports `no_std` without having
  to configure any features.


### Added

- `Deref` derives now dereference to the type in the newtype. So if you have
  `MyBox(Box<i32>)`, dereferencing it will result in a `Box<i32>` not an `i32`.
  To get the old behaviour of forwarding the dereference you can add the
  `#[deref(forward)]` attribute on the struct or field.
- Derives for `AsRef`, `AsMut`, `Sum`, `Product`, `IntoIterator`.
- Choosing the field of a struct for which to derive the newtype derive.
- Ignoring variants of enums when deriving `From`, by using `#[from(ignore)]`.
- Add `#[from(forward)]` attribute for `From` derives. This forwards the `from`
  calls to the fields themselves. So if your field is an `i64` you can call from
  on an `i32` and it will work.
- Add `#[mul(forward)]` and `#[mul_assign(forward)]`, which implement `Mul` and
  `MulAssign` with the semantics as if they were `Add`/`AddAssign`.
- You can use features to cut down compile time of the crate by only compiling
  the code needed for the derives that you use. (see Cargo.toml for the
  features, by default they are all on)
- Add `#[into(owned, ref, ref_mut)]` and `#[try_into(owned, ref, ref_mut)]`
  attributes. These cause the `Into` and `TryInto` derives to also implement
  derives that return references to the inner fields.
- Allow `#[display(fmt="some shared display text for all enum variants {}")]`
  attribute on enum.
- Better bounds inference of `Display` trait.

### Changed

- Remove dependency on `regex` to cut down compile time.
- Use `syn` 1.0

## 0.15.0 - 2019-06-08

### Fixed

- Automatic detection of traits needed for `Display` format strings

## 0.14.0 - 2019-02-02

### Added

- Added `no_std` support

### Changed

- Suppress `unused_variables` warnings in derives

## 0.13.0 - 2018-10-19

### Added

- Extended Display-like derives to support custom formats

### Changed

- Updated to `syn` v0.15

## 0.12.0 - 2018-09-19

### Changed

- Updated to `syn` v0.14, `quote` v0.6 and `proc-macro2` v0.4

## 0.11.0 - 2018-05-12

### Changed

- Updated to latest version of `syn` and `quote`

### Fixed

- Changed some URLs in the docs so they were correct on crates.io and docs.rs
- The `Result` type is now referenced in the derives using its absolute path
  (`::std::result::Result`) to make sure that the derives don't accidentally use
  another `Result` type that is in scope.

## 0.10.0 - 2018-03-29

### Added

- Allow deriving of `TryInto`
- Allow deriving of `Deref`
- Allow deriving of `DerefMut`

## 0.9.0 - 2018-03-18

### Added

- Allow deriving of `Display`, `Binary`, `Octal`, `LowerHex`, `UpperHex`, `LowerExp`, `UpperExp`, `Pointer`
- Allow deriving of `Index`
- Allow deriving of `IndexMut`

### Fixed

- Allow cross crate inlining of derived methods

## 0.8.0 - 2018-03-10

### Added

- Allow deriving of `FromStr`

### Changed

- Updated to latest version of `syn` and `quote`

## 0.7.1 - 2018-01-25

### Fixed

- Add `#[allow(missing_docs)]` to the Constructor definition

## 0.7.0 - 2017-07-25

### Changed

- Changed code to work with newer version of the `syn` library.

## 0.6.2 - 2017-04-23

### Changed

- Deriving `From`, `Into` and `Constructor` now works for empty structs.

## 0.6.1 - 2017-03-08

### Changed

- The `new()` method that is created when deriving `Constructor` is now public.
  This makes it a lot more useful.

## 0.6.0 - 2017-02-20

### Added

- Derives for `Into`, `Constructor` and `MulAssign`-like

### Changed

- `From` is now derived for enum variants with multiple fields.

### Fixed

- Derivations now support generics.

## 0.5.0 - 2017-02-02

### Added

- Lots of docs.
- Derives for `Neg`-like and `AddAssign`-like.

### Changed

- `From` can now be derived for structs with multiple fields.
