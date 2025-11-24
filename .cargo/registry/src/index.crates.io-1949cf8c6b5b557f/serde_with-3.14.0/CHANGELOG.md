# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [3.14.0] - 2025-06-30

### Added

* Add support for `Range`, `RangeFrom`, `RangeTo`, `RangeInclusive` (#851)
    `RangeToInclusive` is currently unsupported by serde.
* Add `schemars` implementations for `Bound`, `Range`, `RangeFrom`, `RangeTo`, `RangeInclusive`.
* Added support for `schemars` v1 under the `schemars_1` feature flag

## [3.13.0] - 2025-06-14

### Added

* Added support for `schemars` v0.9.0 under the `schemars_0_9` feature flag by @swlynch99 (#849)
* Introduce `SerializeDisplayAlt` derive macro (#833)
    An alternative to the `SerializeDisplay` macro except instead of using the
    plain formatting like `format!("{}", ...)`, it serializes with the
    `Formatter::alternate` flag set to true, like `format!("{:#}", ...)`

### Changed

* Generalize `serde_with::rust::unwrap_or_skip` to support deserializing references by @beroal (#832)
* Bump MSRV to 1.71, since that is required for the `jsonschema` dev-dependency.
* Make `serde_conv` available without the `std` feature by @arilou (#839)
* Bump MSRV to 1.74, since that is required for `schemars` v0.9.0 by @swlynch99 (#849)

### Fixed

* Make the `DurationSeconds` types and other variants more accessible even without `std` (#845)

## [3.12.0] - 2024-12-25

### Added

* Add `with_suffix!` macro, which puts a suffix on every struct field by @fgardt (#381/#797)

### Changed

* Reformat all `Cargo.toml` files by @nyurik (#803)
* Better handle internal arithmetic and improve error messages (#809/#810/#811)

### Fixed

* Generated schemas are not valid when using rust style names in ref by @waltronix (#798/#799)

## [3.11.0] - 2024-10-05

### Added

* Add support for `hashbrown` v0.15 (#787/#790)

    This extends the existing support for `hashbrown` v0.14 to the newly released version.

## [3.10.0] - 2024-10-01

### Added

* Add newline separator by @jayvdb (#777)

    The `UnixLineSeparator` and `DosLineSeparator` can be used together with `StringWithSeparator`.

### Fixed

* Proper handling of `cfg_attr` in the `serde_as` macro by @sivizius (#782)

    This allows to parse more valid forms of the `cfg_attr` macro, including multiple values and attribute that do not follow the `key = value` schema.

## [3.9.0] - 2024-07-14

### Added

* Deserialize a map` and skip all elements failing to deserialize by @johnmave126 (#763)

    `MapSkipError` acts like a map (`HashMap`/`BTreeMap`), but keys or values that fail to deserialize, like are ignored.

    For formats with heterogeneously typed maps, we can collect only the elements where both key and value are deserializable.
    This is also useful in conjunction to `#[serde(flatten)]` to ignore some entries when capturing additional fields.

    ```text
    // JSON
    "value": {"0": "v0", "5": "v5", "str": "str", "10": 2},

    // Rust
    #[serde_as(as = "MapSkipError<DisplayFromStr, _>")]
    value: BTreeMap<u32, String>,

    // Only deserializes entries with a numerical key and a string value, i.e.,
    {0 => "v0", 5 => "v5"}
    ```

## [3.8.3] - 2024-07-03

### Fixed

* Fix compile issues when dependency `schemars_0_8` is used with the `preserve_order` features (#762)

## [3.8.2] - 2024-06-30

### Changed

* Bump MSRV to 1.67, since that is required for the `time` dependency.
    The `time` version needed to be updated for nightly compatibility.

### Fixed

* Implement `JsonSchemaAs` for `OneOrMany` instead of `JsonSchema` by @swlynch99 (#760)

## [3.8.1] - 2024-04-28

### Fixed

* Do not emit `schemars(deserialize_with = "...")` annotations, as `schemars` does not support them (#735)
    Thanks to @sivizius for reporting the issue.

## [3.8.0] - 2024-04-24

### Added

* Implement (De)Serialization for Pinned Smart Pointers by @Astralchroma (#733)
* Implement `JsonSchemaAs` for `PickFirst` by @swlynch99  (#721)

### Changed

* Bump `base64` dependency to v0.22 (#724)
* Update dev dependencies

### Fixed

* `serde_conv` regressed and triggered `clippy::ptr_arg` and add test to prevent future problems. (#731)

## [3.7.0] - 2024-03-11

### Added

* Implement `JsonSchemaAs` for `EnumMap` by @swlynch99 (#697)
* Implement `JsonSchemaAs` for `IfIsHumanReadable` by @swlynch99 (#717)
* Implement `JsonSchemaAs` for `KeyValueMap` by @swlynch99 (#713)
* Implement `JsonSchemaAs` for `OneOrMany` by @swlynch99 (#719)

### Fixed

* Detect conflicting `schema_with` attributes on fields with `schemars` annotations by @swlynch99 (#715)
    This extends the existing avoidance mechanism to a new variant fixing #712.

## [3.6.1] - 2024-02-08

### Changed

* Eliminate dependency on serde's "derive" feature by @dtolnay (#694)
    This allows parallel compilation of `serde` and `serde_derive` which can speed up the wallclock time.
    It requires that downstream crates do not use the "derive" feature either.

## [3.6.0] - 2024-01-30

### Added

* Add `IfIsHumanReadable` for conditional implementation by @irriden (#690)
    Used to specify different transformations for text-based and binary formats.
* Add more `JsonSchemaAs` impls for all `Duration*` and `Timestamp*` adaptors by @swlynch99 (#685)

### Changed

* Bump MSRV to 1.65, since that is required for the `regex` dependency.

## [3.5.1] - 2024-01-23

### Fixed

* The `serde_as` macro now better detects existing `schemars` attributes on fields and incorporates them (#682)
    This avoids errors on existing `#[schemars(with = ...)]` annotations.

## [3.5.0] - 2024-01-20

### Added

* Support for `schemars` integration added by @swlynch99 (#666)
    The support uses a new `Schema` top-level item which implements `JsonSchema`
    The `serde_as` macro can now detect `schemars` usage and emits matching annotations for all fields with `serde_as` attribute.
    Many types of this crate come already with support for the `schemars`, but support is not complete and will be extended over time.

## [3.4.0] - 2023-10-17

* Lower minimum required serde version to 1.0.152 (#653)
    Thanks to @banool for submitting the PR.

    This allows people that have a problem with 1.0.153 to still use `serde_with`.
* Add support for `core::ops::Bound` (#655)
    Thanks to @qsantos for submitting the PR.

## [3.3.0] - 2023-08-19

### Added

* Support the `hashbrown` type `HashMap` and `HashSet` (#636, #637)
    Thanks to @OliverNChalk for raising the issue and submitting a PR.

    This extends the existing support for `HashMap`s and `HashSet`s to the `hashbrown` crate v0.14.
    The same conversions as for the `std` and `indexmap` types are available, like general support for `#[serde_as]` and converting it to/from sequences or maps.

### Changed

* Generalize some trait bounds for `DeserializeAs` implementations

    While working on #637, it came to light that some macros for generating `DeserializeAs` implementations were not as generic as they could.
    This means they didn't work with custom hasher types, but only the default hashers.
    This has now been fixed and custom hashers should work better, as long as they implement `BuildHasher + Default`.

* (internal) Change how features are documented (#639)

    This change moves the feature documentation into `Cargo.toml` in a format that can be read by lib.rs.
    It will improve the generated features documentation there.
    The page with all features remains in the guide but is now generated from the `Cargo.toml` information.

## [3.2.0] - 2023-08-04

### Added

* Add optional support for indexmap v2 (#621)
    Support for v1 is already available using the `indexmap_1` feature.
    This adds identical support for v2 of indexmap using the `indexmap_2` feature.

### Changed

* Bump MSRV to 1.64, since that is required for the indexmap v2 dependency.

### Fixed

* Prevent panics when deserializing `i64::MIN` using `TimestampSeconds<i64>` (#632, #633)
    Thanks to @hollmmax for reporting and fixing the issue.

## [3.1.0] - 2023-07-17

### Added

* Add `FromIntoRef` and `TryFromIntoRef` (#618)
    Thanks to @oblique for submitting the PR.

    The new types are similar to the existing `FromInto` and `TryFromInto` types.
    They behave different during serialization, allowing the removal of the `Clone` bound on their `SerializeAs` trait implementation

### Changed

* Improve documentation about cfg-gating `serde_as` (#607)
* Bump MSRV to 1.61 because that is required by the crate `cfg_eval`.

## [3.0.0] - 2023-05-01

This breaking release should not impact most users.
It only affects custom character sets used for base64 of which there are no instances of on GitHub.

### Changed

* Upgrade base64 to v0.21 (#543)
    Thanks to @jeff-hiner for submitting the PR.

    Remove support for custom character sets.
    This is technically a breaking change.
    A code search on GitHub revealed no instances of anyone using that, and `serde_with` ships with many predefined character sets.
    The removal means that future base64 upgrade will no longer be breaking changes.

## [2.3.3] - 2023-04-27

### Changed

* Update `syn` to v2 and `darling` to v0.20 (#578)
    Update proc-macro dependencies.
    This change should have no impact on users, but now uses the same dependency as `serde_derive`.

## [2.3.2] - 2023-04-05

### Changed

* Improve the error message when deserializing `OneOrMany` or `PickFirst` fails.
    It now includes the original error message for each of the individual variants.
    This is possible by dropping untagged enums as the internal implementations, since they will likely never support this, as these old PRs show [serde#2376](https://github.com/serde-rs/serde/pull/2376) and [serde#1544](https://github.com/serde-rs/serde/pull/1544).

    The new errors look like:

    ```text
    OneOrMany could not deserialize any variant:
      One: invalid type: map, expected u32
      Many: invalid type: map, expected a sequence
    ```

    ```text
    PickFirst could not deserialize any variant:
      First: invalid type: string "Abc", expected u32
      Second: invalid digit found in string
    ```

### Fixed

* Specify the correct minimum serde version as dependency. (#588)
    Thanks to @nox for submitting a PR.

## [2.3.1] - 2023-03-10

### Fixed

* Undo the changes to the trait bound for `Seq`. (#570, #571)
    The new implementation caused issues with serialization formats that require the sequence length beforehand.
    It also caused problems such as that certain attributes which worked before no longer worked, due to a mismatching number of references.

    Thanks to @stefunctional for reporting and for @stephaneyfx for providing a test case.

## [2.3.0] - 2023-03-09

### Added

* Add `serde_as` compatible versions for the existing duplicate key and value handling. (#534)
    The new types `MapPreventDuplicates`, `MapFirstKeyWins`, `SetPreventDuplicates`, and `SetLastValueWins` can replace the existing modules `maps_duplicate_key_is_error`, `maps_first_key_wins`, `sets_duplicate_value_is_error`, and `sets_last_value_wins`.
* Added a new `KeyValueMap` type using the map key as a struct field. (#341)
    This conversion is useful for maps, where an ID value is the map key, but the ID should become part of a single struct.
    The conversion allows this, by using a special field named `$key$`.

    This conversion is possible for structs and maps, using the `$key$` field.
    Tuples, tuple structs, and sequences are supported by turning the first value into the map key.

    Each of the `SimpleStruct`s

    ```rust
    // Somewhere there is a collection:
    // #[serde_as(as = "KeyValueMap<_>")]
    // Vec<SimpleStruct>,

    #[derive(Serialize, Deserialize)]
    struct SimpleStruct {
        b: bool,
        // The field named `$key$` will become the map key
        #[serde(rename = "$key$")]
        id: String,
        i: i32,
    }
    ```

    will turn into a JSON snippet like this.

    ```json
    "id-0000": {
      "b": false,
      "i": 123
    },
    ```

### Changed

* Relax the trait bounds of `Seq` to allow for more custom types. (#565)
    This extends the support beyond tuples.

### Fixed

* `EnumMap` passes the `human_readable` status of the `Serializer` to more places.
* Support `alloc` on targets without `target_has_atomic = "ptr"`. (#560)
    Thanks to @vembacher for reporting and fixing the issue.

## [2.2.0] - 2023-01-09

### Added

* Add new `Map` and `Seq` types for converting between maps and tuple lists. (#527)

    The behavior is not new, but already present using `BTreeMap`/`HashMap` or `Vec`.
    However, the new types `Map` and `Seq` are also available on `no_std`, even without the `alloc` feature.

### Changed

* Pin the `serde_with_macros` dependency to the same version as the main crate.
    This simplifies publishing and ensures a compatible version is always picked.

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

### Added

* `time` added support for the well-known `Iso8601` format.
    This extends the existing support of `Rfc2822` and `Rfc3339`.

### Changed

* Warn if `serde_as` is used on an enum variant.
    Attributes on enum variants were never supported.
    But `#[serde(with = "...")]` can be added on variants, such that some confusion can occur when migration ([#499](https://github.com/jonasbb/serde_with/issues/499)).

### Note

A cargo bug ([cargo#10801](https://github.com/rust-lang/cargo/issues/10801)) means that upgrading from v1 to v2 may add unnecessary crates to the `Cargo.lock` file.
A diff of the lock-file makes it seem that `serde_with` depends on new crates, even though these crates are unused and will not get compiled or linked.
However, tools consuming `Cargo.lock` or `cargo metadata` might give wrong results.

## [2.0.0] - 2022-07-17

### Added

* Make `JsonString<T>` smarter by allowing nesting `serde_as` definitions.
    This allows applying custom serialization logic before the value gets converted into a JSON string.

    ```rust
    // Rust
    #[serde_as(as = "JsonString<Vec<(JsonString, _)>>")]
    value: BTreeMap<[u8; 2], u32>,

    // JSON
    {"value":"[[\"[1,2]\",3],[\"[4,5]\",6]]"}
    ```

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
* `NoneAsEmptyString` and `string_empty_as_none` use a different serialization bound (#388).

    Both types used `AsRef<str>` as the serialization bound.
    This is limiting for non-string types like `Option<i32>`.
    The deserialization often was already more flexible, due to the `FromStr` bound.

    For most std types this should have little impact, as the types implementing `AsRef<str>` mostly implement `Display`, too, such as `String`, `Cow<str>`, or `Rc<str>`.
* Bump MSRV to 1.60. This is required for the optional dependency feature syntax in cargo.

### Removed

* Remove old module-based conversions.

    The newer `serde_as` based conversions are preferred.

    * `seq_display_fromstr`: Use `DisplayFromStr` in combination with your container type:

        ```rust
        #[serde_as(as = "BTreeSet<DisplayFromStr>")]
        addresses: BTreeSet<Ipv4Addr>,
        #[serde_as(as = "Vec<DisplayFromStr>")]
        bools: Vec<bool>,
        ```

    * `tuple_list_as_map`: Use `BTreeMap` on a `Vec` of tuples:

        ```rust
        #[serde_as(as = "BTreeMap<_, _>")] // HashMap will also work
        s: Vec<(i32, String)>,
        ```

    * `map_as_tuple_list` can be replaced with `#[serde_as(as = "Vec<(_, _)>")]`.
    * `display_fromstr` can be replaced with `#[serde_as(as = "DisplayFromStr")]`.
    * `bytes_or_string` can be replaced with `#[serde_as(as = "BytesOrString")]`.
    * `default_on_error` can be replaced with `#[serde_as(as = "DefaultOnError")]`.
    * `default_on_null` can be replaced with `#[serde_as(as = "DefaultOnNull")]`.
    * `string_empty_as_none` can be replaced with `#[serde_as(as = "NoneAsEmptyString")]`.
    * `StringWithSeparator` can now only be used in `serde_as`.
        The definition of the `Separator` trait and its implementations have been moved to the `formats` module.
    * `json::nested` can be replaced with `#[serde_as(as = "json::JsonString")]`.

* Remove previously deprecated modules.

    * `sets_first_value_wins`
    * `btreemap_as_tuple_list` and `hashmap_as_tuple_list` can be replaced with `#[serde_as(as = "Vec<(_, _)>")]`.

### Note

A cargo bug ([cargo#10801](https://github.com/rust-lang/cargo/issues/10801)) means that upgrading from v1 to v2 may add unnecessary crates to the `Cargo.lock` file.
A diff of the lock-file makes it seem that `serde_with` depends on new crates, even though these crates are unused and will not get compiled or linked.

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
* `NoneAsEmptyString` and `string_empty_as_none` use a different serialization bound (#388).

    Both types used `AsRef<str>` as the serialization bound.
    This is limiting for non-string types like `Option<i32>`.
    The deserialization often was already more flexible, due to the `FromStr` bound.

    For most std types this should have little impact, as the types implementing `AsRef<str>` mostly implement `Display`, too, such as `String`, `Cow<str>`, or `Rc<str>`.
* Bump MSRV to 1.60. This is required for the optional dependency feature syntax in cargo.

### Removed

* Remove old module-based conversions.

    The newer `serde_as` based conversions are preferred.

    * `seq_display_fromstr`: Use `DisplayFromStr` in combination with your container type:

        ```rust
        #[serde_as(as = "BTreeSet<DisplayFromStr>")]
        addresses: BTreeSet<Ipv4Addr>,
        #[serde_as(as = "Vec<DisplayFromStr>")]
        bools: Vec<bool>,
        ```

    * `tuple_list_as_map`: Use `BTreeMap` on a `Vec` of tuples:

        ```rust
        #[serde_as(as = "BTreeMap<_, _>")] // HashMap will also work
        s: Vec<(i32, String)>,
        ```

    * `map_as_tuple_list` can be replaced with `#[serde_as(as = "Vec<(_, _)>")]`.
    * `display_fromstr` can be replaced with `#[serde_as(as = "DisplayFromStr")]`.
    * `bytes_or_string` can be replaced with `#[serde_as(as = "BytesOrString")]`.
    * `default_on_error` can be replaced with `#[serde_as(as = "DefaultOnError")]`.
    * `default_on_null` can be replaced with `#[serde_as(as = "DefaultOnNull")]`.
    * `string_empty_as_none` can be replaced with `#[serde_as(as = "NoneAsEmptyString")]`.
    * `StringWithSeparator` can now only be used in `serde_as`.
        The definition of the `Separator` trait and its implementations have been moved to the `formats` module.
    * `json::nested` can be replaced with `#[serde_as(as = "json::JsonString")]`.

* Remove previously deprecated modules.

    * `sets_first_value_wins`
    * `btreemap_as_tuple_list` and `hashmap_as_tuple_list` can be replaced with `#[serde_as(as = "Vec<(_, _)>")]`.

## [1.14.0] - 2022-05-29

### Added

* Add support for `time` crate v0.3 #450

    `time::Duration` can now be serialized with the `DurationSeconds` and related converters.

    ```rust
    // Rust
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    value: Duration,

    // JSON
    "value": 86400,
    ```

    `time::OffsetDateTime` and `time::PrimitiveDateTime` can now be serialized with the `TimestampSeconds` and related converters.

    ```rust
    // Rust
    #[serde_as(as = "serde_with::TimestampMicroSecondsWithFrac<String>")]
    value: time::PrimitiveDateTime,

    // JSON
    "value": "1000000",
    ```

    `time::OffsetDateTime` can be serialized in string format in different well-known formats.
    Two formats are supported, `time::format_description::well_known::Rfc2822` and `time::format_description::well_known::Rfc3339`.

    ```rust
    // Rust
    #[serde_as(as = "time::format_description::well_known::Rfc2822")]
    rfc_2822: OffsetDateTime,
    #[serde_as(as = "Vec<time::format_description::well_known::Rfc3339>")]
    rfc_3339: Vec<OffsetDateTime>,

    // JSON
    "rfc_2822": "Fri, 21 Nov 1997 09:55:06 -0600",
    "rfc_3339": ["1997-11-21T09:55:06-06:00"],
    ```

* Deserialize `bool` from integers #456 462

    Deserialize an integer and convert it into a `bool`.
    `BoolFromInt<Strict>` (default) deserializes 0 to `false` and `1` to `true`, other numbers are errors.
    `BoolFromInt<Flexible>` deserializes any non-zero as `true`.
    Serialization only emits 0/1.

    ```rust
    // Rust
    #[serde_as(as = "BoolFromInt")] // BoolFromInt<Strict>
    b: bool,

    // JSON
    "b": 1,
    ```

### Changed

* Bump MSRV to 1.53, since the new dependency `time` requires that version.

### Fixed

* Make the documentation clearer by stating that the `#[serde_as]` and `#[skip_serializing_none]` attributes must always be places before `#[derive]`.

## [1.13.0] - 2022-04-23

### Added

* Added support for `indexmap::IndexMap` and `indexmap::IndexSet` types. #431, #436

    Both types are now compatible with these functions: `maps_duplicate_key_is_error`, `maps_first_key_wins`, `sets_duplicate_value_is_error`, `sets_last_value_wins`.
    `serde_as` integration is provided by implementing both `SerializeAs` and `DeserializeAs` for both types.
    `IndexMap`s can also be serialized as a list of types via the `serde_as(as = "Vec<(_, _)>")` annotation.

    All implementations are gated behind the `indexmap` feature.

    Thanks to @jgrund for providing parts of the implementation.

## [1.12.1] - 2022-04-07

### Fixed

* Depend on a newer `serde_with_macros` version to pull in some fixes.
    * Account for generics when deriving implementations with `SerializeDisplay` and `DeserializeFromStr` #413
    * Provide better error messages when parsing types fails #423

## [1.12.0] - 2022-02-07

### Added

* Deserialize a `Vec` and skip all elements failing to deserialize #383

    `VecSkipError` acts like a `Vec`, but elements which fail to deserialize, like the `"Yellow"` are ignored.

    ```rust
    #[derive(serde::Deserialize)]
    enum Color {
        Red,
        Green,
        Blue,
    }
    // JSON
    "colors": ["Blue", "Yellow", "Green"],
    // Rust
    #[serde_as(as = "VecSkipError<_>")]
    colors: Vec<Color>,
    // => vec![Blue, Green]
    ```

    Thanks to @hdhoang for creating the PR.

* Transform between maps and `Vec<Enum>` #375

    The new `EnumMap` type converts `Vec` of enums into a single map.
    The key is the enum variant name, and the value is the variant value.

    ```rust
    // Rust
    VecEnumValues(vec![
        EnumValue::Int(123),
        EnumValue::String("Foo".to_string()),
        EnumValue::Unit,
        EnumValue::Tuple(1, "Bar".to_string()),
        EnumValue::Struct {
            a: 666,
            b: "Baz".to_string(),
        },
    ]

    // JSON
    {
      "Int": 123,
      "String": "Foo",
      "Unit": null,
      "Tuple": [
        1,
        "Bar",
      ],
      "Struct": {
        "a": 666,
        "b": "Baz",
      }
    }
    ```

### Changed

* The `Timestamp*Seconds` and `Timestamp*SecondsWithFrac` types can now be used with `chrono::NaiveDateTime`. #389

## [1.11.0] - 2021-10-18

### Added

* Serialize bytes as base64 encoded strings.  
    The character set and padding behavior can be configured.

    ```rust
    // Rust
    #[serde_as(as = "serde_with::base64::Base64")]
    value: Vec<u8>,
    #[serde_as(as = "Base64<Bcrypt, Unpadded>")]
    bcrypt_unpadded: Vec<u8>,

    // JSON
    "value": "SGVsbG8gV29ybGQ=",
    "bcrypt_unpadded": "QETqZE6eT07wZEO",
    ```

* The minimal supported Rust version (MSRV) is now specified in the `Cargo.toml` via the `rust-version` field. The field is supported in Rust 1.56 and has no effect on versions before.

    More details: https://doc.rust-lang.org/nightly/cargo/reference/manifest.html#the-rust-version-field

### Fixed

* Fixed RUSTSEC-2020-0071 in the `time` v0.1 dependency, but changing the feature flags of the `chrono` dependency. This should not change anything. Crates requiring the `oldtime` feature of `chrono` can enable it separately.
* Allow `HashSet`s with custom hashers to be deserialized when used in combination with `serde_as`.  #408

## [1.10.0] - 2021-09-04

### Added

* Add `BorrowCow` which instructs serde to borrow data during deserialization of `Cow<'_, str>`, `Cow<'_, [u8]>`, or `Cow<'_, [u8; N]>`. (#347)
    The implementation is for [serde#2072](https://github.com/serde-rs/serde/pull/2072#pullrequestreview-735511713) and [serde#2016](https://github.com/serde-rs/serde/issues/2016), about `#[serde(borrow)]` not working for `Option<Cow<'a, str>>`.

    ```rust
    #[serde_as]
    #[derive(Deserialize, Serialize)]
    struct Data<'a> {
        #[serde_as(as = "Option<[BorrowCow; 1]>")]
        nested: Option<[Cow<'a, str>; 1]>,
    }
    ```

    The `#[serde(borrow)]` annotation is automatically added by the `#[serde_as]` attribute.

### Changed

* Bump MSRV to 1.46, since the dev-dependency `bitflags` requires that version now.
* `flattened_maybe!` no longer requires the `serde_with` crate to be available with a specific name.
    This allows renaming the crate or using `flattened_maybe!` through a re-export without any complications.

## [1.9.4] - 2021-06-18

### Fixed

* `with_prefix!` now supports an optional visibility modifier. (#327, #328)  
    If not specified `pub(self)` is assumed.

    ```rust
    with_prefix!(prefix_active "active_");                   // => mod {...}
    with_prefix!(pub prefix_active "active_");               // => pub mod {...}
    with_prefix!(pub(crate) prefix_active "active_");        // => pub(crate) mod {...}
    with_prefix!(pub(in other_mod) prefix_active "active_"); // => pub(in other_mod) mod {...}
    ```

    Thanks to @elpiel for raising and fixing the issue.

## [1.9.3] - 2021-06-14

### Added

* The `Bytes` type now supports borrowed and Cow arrays of fixed size (requires Rust 1.51+)

    ```rust
    #[serde_as(as = "Bytes")]
    #[serde(borrow)]
    borrowed_array: &'a [u8; 15],
    #[serde_as(as = "Bytes")]
    #[serde(borrow)]
    cow_array: Cow<'a, [u8; 15]>,
    ```

    Note: For borrowed arrays, the used Deserializer needs to support Serde's 0-copy deserialization.

## [1.9.2] - 2021-06-07

### Fixed

* Suppress clippy warnings, which can occur while using `serde_conv` (#320)
    Thanks to @mkroening for reporting and fixing the issue.

## [1.9.1] - 2021-05-15

### Changed

* `NoneAsEmptyString`: Deserialize using `FromStr` instead of using `for<'a> From<&'a str>` (#316)
    This will *not* change any behavior when applied to a field of type `Option<String>` as used in the documentation.
    Thanks to @mkroening for finding and fixing the issue.

## [1.9.0] - 2021-05-09

### Added

* Added `FromInto` and `TryFromInto` adapters, which enable serialization by converting into a proxy type.

    ```rust
    // Rust
    #[serde_as(as = "FromInto<(u8, u8, u8)>")]
    value: Rgb,

    impl From<(u8, u8, u8)> for Rgb { ... }
    impl From<Rgb> for (u8, u8, u8) { ... }

    // JSON
    "value": [128, 64, 32],
    ```

* New `serde_conv!` macro to create conversion types with reduced boilerplate.
    The generated types can be used with `#[serde_as]` or serde's with-attribute.

    ```rust
    serde_with::serde_conv!(
        RgbAsArray,
        Rgb,
        |rgb: &Rgb| [rgb.red, rgb.green, rgb.blue],
        |value: [u8; 3]| -> Result<_, std::convert::Infallible> {
            Ok(Rgb {
                red: value[0],
                green: value[1],
                blue: value[2],
            })
        }
    );
    ```

## [1.8.1] - 2021-04-19

### Added

* The `hex::Hex` type also works for u8-arrays on Rust 1.48.
    Thanks to @TheAlgorythm for raising and fixing the issue.

## [1.8.0] - 2021-03-30

### Added

* Added `PickFirst` adapter for `serde_as`. [#291]
    It allows deserializing from multiple different forms.
    Deserializing a number from either a number or string can be implemented like:

    ```rust
    #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
    value: u32,
    ```

* Implement `SerializeAs`/`DeserializeAs` for more wrapper types. [#288], [#293]
    This now supports:
    * `Arc`, `sync::Weak`
    * `Rc`, `rc::Weak`
    * `Cell`, `RefCell`
    * `Mutex`, `RwLock`
    * `Result`

[#288]: https://github.com/jonasbb/serde_with/issues/288
[#291]: https://github.com/jonasbb/serde_with/issues/291
[#293]: https://github.com/jonasbb/serde_with/issues/293

### Changed

* Add a new `serde_with::rust::map_as_tuple_list` module as a replacement for `serde_with::rust::btreemap_as_tuple_list` and `serde_with::rust::hashmap_as_tuple_list`.
    The new module uses `IntoIterator` and `FromIterator` as trait bound making it usable in more situations.
    The old names continue to exist but are marked as deprecated.

### Deprecated

* Deprecated the module names `serde_with::rust::btreemap_as_tuple_list` and `serde_with::rust::hashmap_as_tuple_list`.
    You can use `serde_with::rust::map_as_tuple_list` as a replacement.

### Fixed

* Implement `Timestamp*Seconds` and `Duration*Seconds` also for chrono types.
    This closes [#194]. This was incompletely implemented in [#199].

[#194]: https://github.com/jonasbb/serde_with/issues/194
[#199]: https://github.com/jonasbb/serde_with/issues/199

## [1.7.0] - 2021-03-24

### Added

* Add support for arrays of arbitrary size. ([#272])
    This feature requires Rust 1.51+.

    ```rust
    // Rust
    #[serde_as(as = "[[_; 64]; 33]")]
    value: [[u8; 64]; 33],

    // JSON
    "value": [[0,0,0,0,0,...], [0,0,0,...], ...],
    ```

    Mapping of arrays was available before, but limited to arrays of length 32.
    All conversion methods are available for the array elements.

    This is similar to the existing [`serde-big-array`] crate with three important improvements:

    1. Support for the `serde_as` annotation.
    2. Supports non-copy elements (see [serde-big-array#6][serde-big-array-copy]).
    3. Supports arbitrary nestings of arrays (see [serde-big-array#7][serde-big-array-nested]).

[#272]: https://github.com/jonasbb/serde_with/pull/272
[`serde-big-array`]: https://crates.io/crates/serde-big-array
[serde-big-array-copy]: https://github.com/est31/serde-big-array/issues/6
[serde-big-array-nested]: https://github.com/est31/serde-big-array/issues/7

* Arrays with tuple elements can now be deserialized from a map. ([#272])
    This feature requires Rust 1.51+.

    ```rust
    // Rust
    #[serde_as(as = "BTreeMap<_, _>")]
    value: [(String, u16); 3],

    // JSON
    "value": {
        "a": 1,
        "b": 2,
        "c": 3
    },
    ```

* The `Bytes` type is heavily inspired by `serde_bytes` and ports it to the `serde_as` system. ([#277])

    ```rust
    #[serde_as(as = "Bytes")]
    value: Vec<u8>,
    ```

    Compared to `serde_bytes` these improvements are available

    1. Integration with the `serde_as` annotation (see [serde-bytes#14][serde-bytes-complex]).
    2. Implementation for arrays of arbitrary size (Rust 1.51+) (see [serde-bytes#26][serde-bytes-arrays]).

[#277]: https://github.com/jonasbb/serde_with/pull/277
[serde-bytes-complex]: https://github.com/serde-rs/bytes/issues/14
[serde-bytes-arrays]: https://github.com/serde-rs/bytes/issues/26

* The `OneOrMany` type allows deserializing a `Vec` from either a single element or a sequence. ([#281])

    ```rust
    #[serde_as(as = "OneOrMany<_>")]
    cities: Vec<String>,
    ```

    This allows deserializing from either `cities: "Berlin"` or `cities: ["Berlin", "Paris"]`.
    The serialization can be configured to always emit a list with `PreferMany` or emit a single element with `PreferOne`.

[#281]: https://github.com/jonasbb/serde_with/pull/281

## [1.6.4] - 2021-02-16

### Fixed

* Fix compiling when having a struct field without the `serde_as` annotation by updating `serde_with_macros`.
    This broke in 1.4.0 of `serde_with_macros`. [#267](https://github.com/jonasbb/serde_with/issues/267)

## [1.6.3] - 2021-02-15

### Changed

* Bump macro crate dependency (`serde_with_macros`) to 1.4.0 to pull in those improvements.

## [1.6.2] - 2021-01-30

### Added

* New function `serde_with::rust::deserialize_ignore_any`.
    This function allows deserializing any data and returns the default value of the type.
    This can be used in conjunction with `#[serde(other)]` to allow deserialization of unknown data carrying enum variants.

    Thanks to @lovasoa for suggesting and implementing it.

## [1.6.1] - 2021-01-24

### Added

* Add new types similar to `DurationSeconds` and `TimestampSeconds` but for base units of milliseconds, microseconds, and nanoseconds.
    The `*WithFrac` variants also exist.
* Add `SerializeAs` implementation for references.

### Changed

* Release `Sized` trait bound from `As`, `Same`, `SerializeAs`, and `SerializeAsWrap`.
    Only the `serialize` part is relaxed.

## [1.6.0] - 2020-11-22

### Added

* Add `DefaultOnNull` as the equivalent for `rust::default_on_null` but for the `serde_as` system.
* Support specifying a path to the `serde_with` crate for the `serde_as` and derive macros.
    This is useful when using crate renaming in Cargo.toml or while re-exporting the macros.

    Many thanks to @tobz1000 for raising the issue and contributing fixes.

### Changed

* Bump minimum supported rust version to 1.40.0

## [1.5.1] - 2020-10-07

### Fixed

* Depend on serde with the `derive` feature enabled.
    The `derive` feature is required to deserialize untagged enums which are used in the `DefaultOnError` helpers.
    This fixes compilation of `serde_with` in scenarios where no other crate enables the `derive` feature.

## [1.5.0] - 2020-10-01

### Added

* The largest addition to this release is the addition of the `serde_as` de/serialization scheme.
    Its goal is to be a more flexible replacement to serde's `with` annotation, by being more composable than before.
    No longer is it a problem to add a custom de/serialization adapter is the type is within an `Option` or a `Vec`.

    Thanks to `@markazmierczak` for the design of the trait without whom this wouldn't be possible.

    More details about this new scheme can be found in the also new [user guide](https://docs.rs/serde_with/1.5.0/serde_with/guide/index.html)
* This release also features a detailed user guide.
    The guide focuses more on how to use this crate by providing examples.
    For example, it includes a section about the available feature flags of this crate and how you can migrate to the shiny new `serde_as` scheme.
* The crate now features de/serialization adaptors for the std and `chrono` `Duration` types. #56 #104
* Add a `hex` module, which allows formatting bytes (i.e. `Vec<u8>`) as a hexadecimal string.
    The formatting supports different arguments how the formatting is happening.
* Add two derive macros, `SerializeDisplay` and `DeserializeFromStr`, which implement the `Serialize`/`Deserialize` traits based on `Display` and `FromStr`.
    This is in addition to the already existing methods like `DisplayFromStr`, which act locally, whereas the derive macros provide the traits expected by the rest of the ecosystem.

    This is part of `serde_with_macros` v1.2.0.
* Added some `serialize` functions to modules which previously had none.
    This makes it easier to use the conversion when also deriving `Serialize`.
    The functions simply pass through to the underlying `Serialize` implementation.
    This affects `sets_duplicate_value_is_error`, `maps_duplicate_key_is_error`, `maps_first_key_wins`, `default_on_error`, and `default_on_null`.
* Added `sets_last_value_wins` as a replacement for `sets_first_value_wins` which is deprecated now.
    The default behavior of serde is to prefer the first value of a set, so the opposite is taking the last value.
* Added `#[serde_as]` compatible conversion methods for serializing durations and timestamps as numbers.
    The four types `DurationSeconds`, `DurationSecondsWithFrac`, `TimestampSeconds`, `TimestampSecondsWithFrac` provide the serialization conversion with optional sub-second precision.
    There is support for `std::time::Duration`, `chrono::Duration`, `std::time::SystemTime` and `chrono::DateTime`.
    Timestamps are serialized as durations since the UNIX epoch.
    The serialization can be customized.
    It supports multiple formats, such as `i64`, `f64`, or `String`, and the deserialization can be tweaked if it should be strict or lenient when accepting formats.

### Changed

* Convert the code to use 2018 edition.
* @peterjoel improved the performance of `with_prefix!`. #101

### Fixed

* The `with_prefix!` macro, to add a string prefixes during serialization, now also works with unit variant enum types. #115 #116
* The `serde_as` macro now supports serde attributes and no longer panic on unrecognized values in the attribute.
    This is part of `serde_with_macros` v1.2.0.

### Deprecated

* Deprecate `sets_first_value_wins`.
    The default behavior of serde is to take the first value, so this module is not necessary.

## [1.5.0-alpha.2] - 2020-08-16

### Added

* Add a `hex` module, which allows formatting bytes (i.e. `Vec<u8>`) as a hexadecimal string.
    The formatting supports different arguments how the formatting is happening.
* Add two derive macros, `SerializeDisplay` and `DeserializeFromStr`, which implement the `Serialize`/`Deserialize` traits based on `Display` and `FromStr`.
    This is in addition to the already existing methods like `DisplayFromStr`, which act locally, whereas the derive macros provide the traits expected by the rest of the ecosystem.

    This is part of `serde_with_macros` v1.2.0-alpha.3.

### Fixed

* The `serde_as` macro now supports serde attributes and no longer panic on unrecognized values in the attribute.
    This is part of `serde_with_macros` v1.2.0-alpha.2.

## [1.5.0-alpha.1] - 2020-06-27

### Added

* The largest addition to this release is the addition of the `serde_as` de/serialization scheme.
    Its goal is to be a more flexible replacement to serde's with annotation, by being more composable than before.
    No longer is it a problem to add a custom de/serialization adapter is the type is within an `Option` or a `Vec`.

    Thanks to `@markazmierczak` for the design of the trait without whom this wouldn't be possible.

    More details about this new scheme can be found in the also new [user guide](https://docs.rs/serde_with/1.5.0-alpha.1/serde_with/guide/index.html)
* This release also features a detailed user guide.
    The guide focuses more on how to use this crate by providing examples.
    For example, it includes a section about the available feature flags of this crate and how you can migrate to the shiny new `serde_as` scheme.
* The crate now features de/serialization adaptors for the std and `chrono`'s `Duration` types. #56 #104

### Changed

* Convert the code to use 2018 edition.
* @peterjoel improved the performance of `with_prefix!`. #101

### Fixed

* The `with_prefix!` macro, to add a string prefixes during serialization, now also works with unit variant enum types. #115 #116

## [1.4.0] - 2020-01-16

### Added

* Add a helper to deserialize a `Vec<u8>` from `String` (#35)
* Add `default_on_error` helper, which turns errors into `Default`s of the type
* Add `default_on_null` helper, which turns `null` values into `Default`s of the type

### Changed

* Bump minimal Rust version to 1.36.0
    * Supports Rust Edition 2018
    * version-sync depends on smallvec, which requires 1.36
* Improved CI pipeline by running `cargo audit` and `tarpaulin` in all configurations now.

## [1.3.1] - 2019-04-09

### Fixed

* Use `serde_with_macros` with proper dependencies specified.

## [1.3.0] - 2019-04-02

### Added

* Add `skip_serializing_none` attribute, which adds `#[serde(skip_serializing_if = "Option::is_none")]` for each Option in a struct.
    This is helpful for APIs which have many optional fields.
    The effect of can be negated by adding `serialize_always` on those fields, which should always be serialized.
    Existing `skip_serializing_if` will never be modified and those fields keep their behavior.

## [1.2.0] - 2019-03-04

### Added

* Add macro helper to support deserializing values with nested or flattened syntax #38
* Serialize tuple list as map helper

### Changed

* Bumped minimal Rust version to 1.30.0

## [1.1.0] - 2019-02-18

### Added

* Serialize HashMap/BTreeMap as a list of tuples

## [1.0.0] - 2019-01-17

### Added

* No changes in this release.
* Bumped version number to indicate the stability of the library.

## [0.2.5] - 2018-11-29

### Added

* Helper which deserializes an empty string as `None` and otherwise uses `FromStr` and `AsRef<str>`.

## [0.2.4] - 2018-11-24

### Added

* De/Serialize sequences by using `Display` and `FromStr` implementations on each element. Contributed by @katyo

## [0.2.3] - 2018-11-08

### Added

* Add missing docs and enable deny missing_docs
* Add badges to Cargo.toml and crates.io

### Changed

* Improve Travis configuration
* Various clippy improvements

## [0.2.2] - 2018-08-05

### Added

* `unwrap_or_skip` allows to transparently serialize the inner part of a `Some(T)`
* Add deserialization helper for sets and maps, inspired by [comment](https://github.com/serde-rs/serde/issues/553#issuecomment-299711855)
    * Create an error if duplicate values for a set are detected
    * Create an error if duplicate keys for a map are detected
    * Implement a "first value wins" strategy for sets/maps.
      This is different to serde's default, which implements a "last value wins" strategy.

## [0.2.1] - 2018-06-05

### Added

* Double Option pattern to differentiate between missing, unset, or existing value
* `with_prefix!` macro, which puts a prefix on every struct field

## [0.2.0] - 2018-05-31

### Added

* Add chrono support: Deserialize timestamps from int, float, and string
* Serialization of embedded JSON strings
* De/Serialization using `Display` and `FromStr` implementations
* String-based collections using `Display` and `FromStr`, allows deserializing "#foo,#bar"

## [0.1.0] - 2017-08-17

### Added

* Reserve name on crates.io
