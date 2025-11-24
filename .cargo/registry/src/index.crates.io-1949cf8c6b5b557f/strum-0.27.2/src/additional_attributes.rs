//! # Documentation for Additional Attributes
//!
//! ## Attributes on Enums
//!
//! Strum supports several custom attributes to modify the generated code. At the enum level, the following attributes are supported:
//!
//! - `#[strum(serialize_all = "case_style")]` attribute can be used to change the case used when serializing to and deserializing
//!   from strings. This feature is enabled by [withoutboats/heck](https://github.com/withoutboats/heck) and supported case styles are:
//!
//!   - `camelCase`
//!   - `PascalCase`
//!   - `kebab-case`
//!   - `snake_case`
//!   - `SCREAMING_SNAKE_CASE`
//!   - `SCREAMING-KEBAB-CASE`
//!   - `lowercase`
//!   - `UPPERCASE`
//!   - `title_case`
//!   - `mixed_case`
//!   - `Train-Case`
//!
//!   ```rust
//!   use strum_macros;
//!   
//!   #[derive(Debug, Eq, PartialEq, strum_macros::Display)]
//!   #[strum(serialize_all = "snake_case")]
//!   enum Brightness {
//!       DarkBlack,
//!       Dim {
//!           glow: usize,
//!       },
//!       #[strum(serialize = "bright")]
//!       BrightWhite,
//!   }
//!   
//!   assert_eq!(
//!       String::from("dark_black"),
//!       Brightness::DarkBlack.to_string().as_ref()
//!   );
//!   assert_eq!(
//!       String::from("dim"),
//!       Brightness::Dim { glow: 0 }.to_string().as_ref()
//!   );
//!   assert_eq!(
//!       String::from("bright"),
//!       Brightness::BrightWhite.to_string().as_ref()
//!   );
//!   ```
//!
//! - You can also apply the `#[strum(ascii_case_insensitive)]` attribute to the enum,
//!   and this has the same effect of applying it to every variant.
//!
//! ## Attributes on Variants
//!
//! Custom attributes are applied to a variant by adding `#[strum(parameter="value")]` to the variant.
//!
//! - `serialize="..."`: Changes the text that `FromStr()` looks for when parsing a string. This attribute can
//!    be applied multiple times to an element and the enum variant will be parsed if any of them match.
//!
//! - `to_string="..."`: Similar to `serialize`. This value will be included when using `FromStr()`. More importantly,
//!    this specifies what text to use when calling `variant.to_string()` with the `Display` derivation, or when calling `variant.as_ref()` with `AsRefStr`.
//!
//! - `default`: Applied to a single variant of an enum. The variant must be a Tuple-like
//!    variant with a single piece of data that can be create from a `&str` i.e. `T: From<&str>`.
//!    The generated code will now return the variant with the input string captured as shown below
//!    instead of failing.
//!
//!     ```text
//!     // Replaces this:
//!     _ => Err(strum::ParseError::VariantNotFound)
//!     // With this in generated code:
//!     default => Ok(Variant(default.into()))
//!     ```
//!     The plugin will fail if the data doesn't implement From<&str>. You can only have one `default`
//!     on your enum.
//!
//! - `transparent`: Signals that the inner field's implementation should be used, instead of generating
//!    one for this variant. Only applicable to enum variants with a single field. Compatible with the
//!    `AsRefStr`, `Display` and `IntoStaticStr` derive macros. Note that `IntoStaticStr` has a few restrictions,
//!    the value must be `'static` and `const_into_str` is not supported in combination with `transparent` b/c
//!    transparent relies on a call on `From::from(variant)`.
//!
//! - `disabled`: removes variant from generated code.
//!
//! - `ascii_case_insensitive`: makes the comparison to this variant case insensitive (ASCII only).
//!   If the whole enum is marked `ascii_case_insensitive`, you can specify `ascii_case_insensitive = false`
//!   to disable case insensitivity on this variant.
//!
//! - `message=".."`: Adds a message to enum variant. This is used in conjunction with the `EnumMessage`
//!    trait to associate a message with a variant. If `detailed_message` is not provided,
//!    then `message` will also be returned when `get_detailed_message` is called.
//!
//! - `detailed_message=".."`: Adds a more detailed message to a variant. If this value is omitted, then
//!    `message` will be used in it's place.
//!
//! - Structured documentation, as in `/// ...`: If using `EnumMessage`, is accessible via get_documentation().
//!
//! - `props(key="value")`: Enables associating additional information with a given variant.
