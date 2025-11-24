//! Crate for changing case of Rust identifiers.
//!
//! # Features
//! * Supports `snake_case`, `lowercase`, `camelCase`, 
//!   `PascalCase`, `SCREAMING_SNAKE_CASE`, and `kebab-case`
//! * Rename variants, and fields
//! 
//! # Examples
//! ```rust
//! use ident_case::RenameRule;
//!
//! assert_eq!("helloWorld", RenameRule::CamelCase.apply_to_field("hello_world"));
//!
//! assert_eq!("i_love_serde", RenameRule::SnakeCase.apply_to_variant("ILoveSerde"));
//! ```

// Copyright 2017 Serde Developers
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::ascii::AsciiExt;
use std::str::FromStr;

use self::RenameRule::*;

/// A casing rule for renaming Rust identifiers.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RenameRule {
    /// No-op rename rule.
    None,
    /// Rename direct children to "lowercase" style.
    LowerCase,
    /// Rename direct children to "PascalCase" style, as typically used for enum variants.
    PascalCase,
    /// Rename direct children to "camelCase" style.
    CamelCase,
    /// Rename direct children to "snake_case" style, as commonly used for fields.
    SnakeCase,
    /// Rename direct children to "SCREAMING_SNAKE_CASE" style, as commonly used for constants.
    ScreamingSnakeCase,
    /// Rename direct children to "kebab-case" style.
    KebabCase,
}

impl RenameRule {
    /// Change case of a `PascalCase` variant.
    pub fn apply_to_variant<S: AsRef<str>>(&self, variant: S) -> String {
        
        let variant = variant.as_ref();
        match *self {
            None | PascalCase => variant.to_owned(),
            LowerCase => variant.to_ascii_lowercase(),
            CamelCase => variant[..1].to_ascii_lowercase() + &variant[1..],
            SnakeCase => {
                let mut snake = String::new();
                for (i, ch) in variant.char_indices() {
                    if i > 0 && ch.is_uppercase() {
                        snake.push('_');
                    }
                    snake.push(ch.to_ascii_lowercase());
                }
                snake
            }
            ScreamingSnakeCase => SnakeCase.apply_to_variant(variant).to_ascii_uppercase(),
            KebabCase => SnakeCase.apply_to_variant(variant).replace('_', "-"),
        }
    }

    /// Change case of a `snake_case` field.
    pub fn apply_to_field<S: AsRef<str>>(&self, field: S) -> String {
        
        let field = field.as_ref();
        match *self {
            None | LowerCase | SnakeCase => field.to_owned(),
            PascalCase => {
                let mut pascal = String::new();
                let mut capitalize = true;
                for ch in field.chars() {
                    if ch == '_' {
                        capitalize = true;
                    } else if capitalize {
                        pascal.push(ch.to_ascii_uppercase());
                        capitalize = false;
                    } else {
                        pascal.push(ch);
                    }
                }
                pascal
            }
            CamelCase => {
                let pascal = PascalCase.apply_to_field(field);
                pascal[..1].to_ascii_lowercase() + &pascal[1..]
            }
            ScreamingSnakeCase => field.to_ascii_uppercase(),
            KebabCase => field.replace('_', "-"),
        }
    }
}

impl FromStr for RenameRule {
    type Err = ();

    fn from_str(rename_all_str: &str) -> Result<Self, Self::Err> {
        match rename_all_str {
            "lowercase" => Ok(LowerCase),
            "PascalCase" => Ok(PascalCase),
            "camelCase" => Ok(CamelCase),
            "snake_case" => Ok(SnakeCase),
            "SCREAMING_SNAKE_CASE" => Ok(ScreamingSnakeCase),
            "kebab-case" => Ok(KebabCase),
            _ => Err(()),
        }
    }
}

impl Default for RenameRule {
    fn default() -> Self {
        RenameRule::None
    }
}

#[cfg(test)]
mod tests {
    use super::RenameRule::*;

    #[test]
    fn rename_variants() {
        for &(original, lower, camel, snake, screaming, kebab) in
            &[
                ("Outcome", "outcome", "outcome", "outcome", "OUTCOME", "outcome"),
                ("VeryTasty", "verytasty", "veryTasty", "very_tasty", "VERY_TASTY", "very-tasty"),
                ("A", "a", "a", "a", "A", "a"),
                ("Z42", "z42", "z42", "z42", "Z42", "z42"),
            ] {
            assert_eq!(None.apply_to_variant(original), original);
            assert_eq!(LowerCase.apply_to_variant(original), lower);
            assert_eq!(PascalCase.apply_to_variant(original), original);
            assert_eq!(CamelCase.apply_to_variant(original), camel);
            assert_eq!(SnakeCase.apply_to_variant(original), snake);
            assert_eq!(ScreamingSnakeCase.apply_to_variant(original), screaming);
            assert_eq!(KebabCase.apply_to_variant(original), kebab);
        }
    }

    #[test]
    fn rename_fields() {
        for &(original, pascal, camel, screaming, kebab) in
            &[
                ("outcome", "Outcome", "outcome", "OUTCOME", "outcome"),
                ("very_tasty", "VeryTasty", "veryTasty", "VERY_TASTY", "very-tasty"),
                ("_leading_under", "LeadingUnder", "leadingUnder", "_LEADING_UNDER", "-leading-under"),
                ("double__under", "DoubleUnder", "doubleUnder", "DOUBLE__UNDER", "double--under"),
                ("a", "A", "a", "A", "a"),
                ("z42", "Z42", "z42", "Z42", "z42"),
            ] {
            assert_eq!(None.apply_to_field(original), original);
            assert_eq!(PascalCase.apply_to_field(original), pascal);
            assert_eq!(CamelCase.apply_to_field(original), camel);
            assert_eq!(SnakeCase.apply_to_field(original), original);
            assert_eq!(ScreamingSnakeCase.apply_to_field(original), screaming);
            assert_eq!(KebabCase.apply_to_field(original), kebab);
        }
    }
}