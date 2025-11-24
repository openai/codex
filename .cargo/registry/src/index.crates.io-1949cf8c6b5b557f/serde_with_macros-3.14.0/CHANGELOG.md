# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [3.14.0] - 2025-06-30

### Added

* Added support for `schemars` v1 under the `schemars_1` feature flag

## [3.13.0] - 2025-06-14

### Added

* Introduce `SerializeDisplayAlt` derive macro (#833)
    An alternative to the `SerializeDisplay` macro except instead of using the
    plain formatting like `format!("{}", ...)`, it serializes with the
    `Formatter::alternate` flag set to true, like `format!("{:#}", ...)`

## [3.12.0] - 2024-12-25

No changes.

## [3.11.0] - 2024-10-05

No changes.

## [3.10.0] - 2024-10-01

### Fixed

* Proper handling of `cfg_attr` in the `serde_as` macro by @sivizius (#782)

    This allows to parse more valid forms of the `cfg_attr` macro, including multiple values and attribute that do not follow the `key = value` schema.

## [3.9.0] - 2024-07-14

No changes.

## [3.8.3] - 2024-07-03

No changes.

## [3.8.2] - 2024-06-30

No changes.

## [3.8.1] - 2024-04-28

No changes.

## [3.8.0] - 2024-04-24

No changes.

## [3.7.0] - 2024-03-11

### Fixed

* Detect conflicting `schema_with` attributes on fields with `schemars` annotations by @swlynch99 (#715)
    This extends the existing avoidance mechanism to a new variant fixing #712.

## [3.6.1] - 2024-02-08

No changes.

## [3.6.0] - 2024-01-30

No changes.

## [3.5.1] - 2024-01-23

### Fixed

* The `serde_as` macro now better detects existing `schemars` attributes on fields and incorporates them (#682)
    This avoids errors on existing `#[schemars(with = ...)]` annotations.

## [3.5.0] - 2024-01-20

### Added

* Support for `schemars` integration added by @swlynch99 (#666)
    The `serde_as` macro can now detect `schemars` usage and emits matching annotations for all fields with `serde_as` attribute.

## [3.4.0] - 2023-10-17

No changes.

## [3.3.0] - 2023-08-19

No changes.

## [3.2.0] - 2023-08-04

No changes.

## [3.1.0] - 2023-07-17

No changes.

## [3.0.0] - 2023-05-01

No changes.

## [2.3.3] - 2023-04-27

### Changed

* Update `syn` to v2 and `darling` to v0.20 (#578)
    Update proc-macro dependencies.
    This change should have no impact on users, but now uses the same dependency as `serde_derive`.

## [2.3.2] - 2023-04-05

No changes.

## [2.3.1] - 2023-03-10

No changes.

## [2.3.0] - 2023-03-09

No changes.

## [2.2.0] - 2023-01-09

### Fixed

* `serde_with::apply` had an issue matching types when invisible token groups where in use (#538)
    The token groups can stem from macro_rules expansion, but should be treated mostly transparent.
    The old code required a group to match a group, while now groups are silently removed when checking for type patterns.

## [2.1.0] - 2022-11-16

### Added

* Add new `apply` attribute to simplify repetitive attributes over many fields.
    Multiple rules and multiple attributes can be provided each.

    ```rust
    #[serde_with::apply(
        Option => #[serde(default)] #[serde(skip_serializing_if = "Option::is_none")],
        Option<bool> => #[serde(rename = "bool")],
    )]
    #[derive(serde::Serialize)]
    struct Data {
        a: Option<String>,
        b: Option<u64>,
        c: Option<String>,
        d: Option<bool>,
    }
    ```

    The `apply` attribute will expand into this, applying the attributes to the matching fields:

    ```rust
    #[derive(serde::Serialize)]
    struct Data {
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        a: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        b: Option<u64>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        c: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "bool")]
        d: Option<bool>,
    }
    ```

    The attribute supports field matching using many rules, such as `_` to apply to all fields and partial generics like `Option` to match any `Option` be it `Option<String>`, `Option<bool>`, or `Option<T>`.

### Fixed

* The derive macros `SerializeDisplay` and `DeserializeFromStr` now take better care not to use conflicting names for generic values. (#526)
    All used generics now start with `__` to make conflicts with manually written code unlikely.

    Thanks to @Elrendio for submitting a PR fixing the issue.

## [2.0.1] - 2022-09-09

### Changed

* Warn if `serde_as` is used on an enum variant.
    Attributes on enum variants were never supported.
    But `#[serde(with = "...")]` can be added on variants, such that some confusion can occur when migration ([#499](https://github.com/jonasbb/serde_with/issues/499)).

## [2.0.0] - 2022-07-17

No changes compared to v2.0.0-rc.0.

### Changed

* Make `#[serde_as]` behave more intuitive on `Option<T>` fields.

    The `#[serde_as]` macro now detects if a `#[serde_as(as = "Option<S>")]` is used on a field of type `Option<T>` and applies `#[serde(default)]` to the field.
    This restores the ability to deserialize with missing fields and fixes a common annoyance (#183, #185, #311, #417).
    This is a breaking change, since now deserialization will pass where it did not before and this might be undesired.

    The `Option` field and transformation are detected by directly matching on the type name.
    These variants are detected as `Option`.
    * `Option`
    * `std::option::Option`, with or without leading `::`
    * `core::option::Option`, with or without leading `::`

    If an existing `default` attribute is detected, the attribute is not applied again.
    This behavior can be suppressed by using `#[serde_as(no_default)]` or `#[serde_as(as = "Option<S>", no_default)]`.

### Fixed

* Make the documentation clearer by stating that the `#[serde_as]` and `#[skip_serializing_none]` attributes must always be placed before `#[derive]`.

## [2.0.0-rc.0] - 2022-06-29

### Changed

* Make `#[serde_as]` behave more intuitive on `Option<T>` fields.

    The `#[serde_as]` macro now detects if a `#[serde_as(as = "Option<S>")]` is used on a field of type `Option<T>` and applies `#[serde(default)]` to the field.
    This restores the ability to deserialize with missing fields and fixes a common annoyance (#183, #185, #311, #417).
    This is a breaking change, since now deserialization will pass where it did not before and this might be undesired.

    The `Option` field and transformation are detected by directly matching on the type name.
    These variants are detected as `Option`.
    * `Option`
    * `std::option::Option`, with or without leading `::`
    * `core::option::Option`, with or without leading `::`

    If an existing `default` attribute is detected, the attribute is not applied again.
    This behavior can be suppressed by using `#[serde_as(no_default)]` or `#[serde_as(as = "Option<S>", no_default)]`.

### Fixed

* Make the documentation clearer by stating that the `#[serde_as]` and `#[skip_serializing_none]` attributes must always be placed before `#[derive]`.

## [1.5.2] - 2022-04-07

### Fixed

* Account for generics when deriving implementations with `SerializeDisplay` and `DeserializeFromStr` #413
* Provide better error messages when parsing types fails #423

## [1.5.1] - 2021-10-18

### Added

* The minimal supported Rust version (MSRV) is now specified in the `Cargo.toml` via the `rust-version` field. The field is supported in Rust 1.56 and has no effect on versions before.

    More details: https://doc.rust-lang.org/nightly/cargo/reference/manifest.html#the-rust-version-field

## [1.5.0] - 2021-09-04

### Added

* Add the attribute `#[serde(borrow)]` on a field if `serde_as` is used in combination with the `BorrowCow` type.

## [1.4.2] - 2021-06-07

### Fixed

* Describe how the `serde_as` macro works on a high level.
* The derive macros `SerializeDisplay` and `DeserializeFromStr` were relying on the prelude where they were used.
    Properly name all types and traits required for the expanded code to work.
    The tests were improved to be better able to catch such problems.

## [1.4.2] - 2021-02-16

### Fixed

* Fix compiling when having a struct field without the `serde_as` annotation.
    This broke in 1.4.0 [#267](https://github.com/jonasbb/serde_with/issues/267)

## [1.4.0] - 2021-02-15

### Changed

* Improve error messages when `#[serde_as(..)]` is misused as a field attribute.
    Thanks to @Lehona for reporting the bug in #233.
* Internal cleanup for assembling and parsing attributes during `serde_as` processing.
* Change processing on `#[serde_as(...)]` attributes on fields.

    The attributes will no longer be stripped during proc-macro processing.
    Instead, a private derive macro is applied to the struct/enum which captures them and makes them inert, thus allowing compilation.

    This should have no effect on the generated code and on the runtime behavior.
    It eases integration of third-party crates with `serde_with`, since they can now process the `#[serde_as(...)]` field attributes reliably.
    Before this, it was impossible for derive macros and lead to awkward ordering constraints on the attribute macros.

    Thanks to @Lehona for reporting this problem and to @dtolnay for suggesting the dummy derive macro.

## [1.3.0] - 2020-11-22

### Added

* Support specifying a path to the `serde_with` crate for the `serde_as` and derive macros.
    This is useful when using crate renaming in Cargo.toml or while re-exporting the macros.

    Many thanks to @tobz1000 for raising the issue and contributing fixes.

### Changed

* Bump minimum supported rust version to 1.40.0

## [1.2.2] - 2020-10-06

### Fixed

* @adwhit contributed an improvement to `DeserializeFromStr` which allows it to deserialize from bytes (#186).
    This makes the derived implementation applicable in more situations.

## [1.2.1] - 2020-10-04

### Fixed

* The derive macros `SerializeDisplay` and `DeserializeFromStr` now use the properly namespaced types and traits.
    This solves conflicts with `Result` if `Result` is not `std::result::Result`, e.g., a type alias.
    Additionally, the code assumed that `FromStr` was in scope, which is now also not required.

    Thanks to @adwhit for reporting and fixing the problem in #186.

## [1.2.0] - 2020-10-01

### Added

* Add `serde_as` macro. Refer to the `serde_with` crate for details.
* Add two derive macros, `SerializeDisplay` and `DeserializeFromStr`, which implement the `Serialize`/`Deserialize` traits based on `Display` and `FromStr`.
  This is in addition to the pre-existing methods like `DisplayFromStr`, which act locally, whereas the derive macros provide the traits expected by the rest of the ecosystem.

### Changed

* Convert the code to use 2018 edition.

### Fixed

* The `serde_as` macro now supports serde attributes and no longer panic on unrecognized values in the attribute.

## [1.2.0-alpha.3] - 2020-08-16

### Added

* Add two derive macros, `SerializeDisplay` and `DeserializeFromStr`, which implement the `Serialize`/`Deserialize` traits based on `Display` and `FromStr`.
  This is in addition to the pre-existing methods like `DisplayFromStr`, which act locally, whereas the derive macros provide the traits expected by the rest of the ecosystem.

## [1.2.0-alpha.2] - 2020-08-08

### Fixed

* The `serde_as` macro now supports serde attributes and no longer panic on unrecognized values in the attribute.

## [1.2.0-alpha.1] - 2020-06-27

### Added

* Add `serde_as` macro. Refer to the `serde_with` crate for details.

### Changed

* Convert the code to use 2018 edition.

## [1.1.0] - 2020-01-16

### Changed

* Bump minimal Rust version to 1.36.0 to support Rust Edition 2018
* Improved CI pipeline by running `cargo audit` and `tarpaulin` in all configurations now.

## [1.0.1] - 2019-04-09

### Fixed

* Features for the `syn` dependency were missing.
    This was hidden due to the dev-dependencies whose features leaked into the normal build.

## [1.0.0] - 2019-04-02

Initial Release

### Added

* Add `skip_serializing_none` attribute, which adds `#[serde(skip_serializing_if = "Option::is_none")]` for each Option in a struct.
    This is helpful for APIs which have many optional fields.
    The effect of can be negated by adding `serialize_always` on those fields, which should always be serialized.
    Existing `skip_serializing_if` will never be modified and those fields keep their behavior.
