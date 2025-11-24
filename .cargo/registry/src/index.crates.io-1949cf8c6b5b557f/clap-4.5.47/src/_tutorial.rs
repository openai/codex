// Contributing
//
// New example code:
// - Please update the corresponding section in the derive tutorial
// - Building: They must be added to `Cargo.toml` with the appropriate `required-features`.
// - Testing: Ensure there is a markdown file with [trycmd](https://docs.rs/trycmd) syntax
//
// See also the general CONTRIBUTING

//! ## Tutorial for the Builder API
//!
//! *See the side bar for the Table of Contents*
//!
//! ## Quick Start
//!
//! You can create an application with several arguments using usage strings.
//!
//! First, ensure `clap` is available:
//! ```console
//! $ cargo add clap
//! ```
//!
//! Here is a preview of the type of application you can make:
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/01_quick.rs")]
//! ```
//!
#![doc = include_str!("../examples/tutorial_builder/01_quick.md")]
//!
//! See also
//! - [FAQ: When should I use the builder vs derive APIs?][crate::_faq#when-should-i-use-the-builder-vs-derive-apis]
//! - The [cookbook][crate::_cookbook] for more application-focused examples
//!
//! ## Configuring the Parser
//!
//! You use [`Command`][crate::Command] to start building a parser.
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/02_apps.rs")]
//! ```
//!
#![doc = include_str!("../examples/tutorial_builder/02_apps.md")]
//!
//! You can use [`command!()`][crate::command!] to fill these fields in from your `Cargo.toml`
//! file.  **This requires the [`cargo` feature flag][crate::_features].**
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/02_crate.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/02_crate.md")]
//!
//! You can use [`Command`][crate::Command] methods to change the application level behavior of
//! clap, like [`Command::next_line_help`].
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/02_app_settings.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/02_app_settings.md")]
//!
//! ## Adding Arguments
//!
//! 1. [Positionals](#positionals)
//! 2. [Options](#options)
//! 3. [Flags](#flags)
//! 4. [Required](#required)
//! 5. [Defaults](#defaults)
//! 6. [Subcommands](#subcommands)
//!
//!
//! ### Positionals
//!
//! By default, an [`Arg`] defines a positional argument:
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/03_03_positional.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/03_03_positional.md")]
//!
//! Note that the default [`ArgAction`][crate::ArgAction] is [`Set`][crate::ArgAction::Set].  To
//! accept multiple values, override the [action][Arg::action] with [`Append`][crate::ArgAction::Append]:
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/03_03_positional_mult.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/03_03_positional_mult.md")]
//!
//! ### Options
//!
//! You can name your arguments with a flag:
//! - Intent of the value is clearer
//! - Order doesn't matter
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/03_02_option.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/03_02_option.md")]
//!
//! Note that the default [`ArgAction`][crate::ArgAction] is [`Set`][crate::ArgAction::Set].  To
//! accept multiple occurrences, override the [action][Arg::action] with [`Append`][crate::ArgAction::Append]:
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/03_02_option_mult.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/03_02_option_mult.md")]
//!
//! ### Flags
//!
//! Flags can also be switches that can be on/off:
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/03_01_flag_bool.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/03_01_flag_bool.md")]
//!
//! To accept multiple flags, use [`Count`][crate::ArgAction::Count]:
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/03_01_flag_count.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/03_01_flag_count.md")]
//!
//! ### Required
//!
//! By default, an [`Arg`] is optional which can be changed with
//! [`required`][crate::Arg::required].
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/03_06_required.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/03_06_required.md")]
//!
//! ### Defaults
//!
//! We've previously showed that arguments can be [`required`][crate::Arg::required] or optional.
//! When optional, you work with a `Option` and can `unwrap_or`.  Alternatively, you can set
//! [`Arg::default_value`][crate::Arg::default_value].
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/03_05_default_values.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/03_05_default_values.md")]
//!
//! ### Subcommands
//!
//! Subcommands are defined as [`Command`][crate::Command]s that get added via
//! [`Command::subcommand`][crate::Command::subcommand]. Each instance of a Subcommand can have its
//! own version, author(s), Args, and even its own subcommands.
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/03_04_subcommands.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/03_04_subcommands.md")]
//!
//! ## Validation
//!
//! 1. [Enumerated values](#enumerated-values)
//! 2. [Validated values](#validated-values)
//! 3. [Argument Relations](#argument-relations)
//! 4. [Custom Validation](#custom-validation)
//!
//! An appropriate default parser/validator will be selected for the field's type.  See
//! [`value_parser!`][crate::value_parser!] for more details.
//!
//! ### Enumerated values
//!
//! If you have arguments of specific values you want to test for, you can use the
//! [`PossibleValuesParser`] or [`Arg::value_parser(["val1",
//! ...])`][crate::Arg::value_parser] for short.
//!
//! This allows you to specify the valid values for that argument. If the user does not use one of
//! those specific values, they will receive a graceful exit with error message informing them
//! of the mistake, and what the possible valid values are
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/04_01_possible.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/04_01_possible.md")]
//!
//! When enabling the [`derive` feature][crate::_features], you can use
//! [`ValueEnum`][crate::ValueEnum] to take care of the boiler plate for you, giving the same
//! results.
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/04_01_enum.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/04_01_enum.md")]
//!
//! ### Validated values
//!
//! More generally, you can validate and parse into any data type with [`Arg::value_parser`].
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/04_02_parse.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/04_02_parse.md")]
//!
//! A [custom parser][TypedValueParser] can be used to improve the error messages or provide additional validation:
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/04_02_validate.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/04_02_validate.md")]
//!
//! See [`Arg::value_parser`][crate::Arg::value_parser] for more details.
//!
//! ### Argument Relations
//!
//! You can declare dependencies or conflicts between [`Arg`][crate::Arg]s or even
//! [`ArgGroup`][crate::ArgGroup]s.
//!
//! [`ArgGroup`][crate::ArgGroup]s  make it easier to declare relations instead of having to list
//! each individually, or when you want a rule to apply "any but not all" arguments.
//!
//! Perhaps the most common use of [`ArgGroup`][crate::ArgGroup]s is to require one and *only* one
//! argument to be present out of a given set. Imagine that you had multiple arguments, and you
//! want one of them to be required, but making all of them required isn't feasible because perhaps
//! they conflict with each other.
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/04_03_relations.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/04_03_relations.md")]
//!
//! ### Custom Validation
//!
//! As a last resort, you can create custom errors with the basics of clap's formatting.
//!
//! ```rust
#![doc = include_str!("../examples/tutorial_builder/04_04_custom.rs")]
//! ```
#![doc = include_str!("../examples/tutorial_builder/04_04_custom.md")]
//!
//! ## Testing
//!
//! clap reports most development errors as `debug_assert!`s.  Rather than checking every
//! subcommand, you should have a test that calls
//! [`Command::debug_assert`][crate::Command::debug_assert]:
//! ```rust,no_run
#![doc = include_str!("../examples/tutorial_builder/05_01_assert.rs")]
//! ```
//!
//! ## Next Steps
//!
//! - [Cookbook][crate::_cookbook] for application-focused examples
//! - Explore more features in the [API reference][super]
//!
//! For support, see [Discussions](https://github.com/clap-rs/clap/discussions)
#![allow(unused_imports)]
use crate::builder::*;
