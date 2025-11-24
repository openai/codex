// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

/// Macro used to generate a preference keyword as a struct.
///
/// # Examples
///
/// ```
/// use icu::locale::{
///     extensions::unicode::{Key, Value},
///     preferences::extensions::unicode::struct_keyword,
/// };
///
/// struct_keyword!(
///     CurrencyType,
///     "cu",
///     String,
///     |input: Value| { Ok(Self(input.to_string())) },
///     |input: CurrencyType| {
///         icu::locale::extensions::unicode::Value::try_from_str(
///             input.0.as_str(),
///         )
///         .unwrap()
///     }
/// );
/// ```
#[macro_export]
#[doc(hidden)]
macro_rules! __struct_keyword {
    ($(#[$doc:meta])* $([$derive_attrs:ty])? $name:ident, $ext_key:literal, $value:ty, $try_from:expr, $into:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Eq, PartialEq, Hash)]
        $(#[derive($derive_attrs)])?
        #[allow(clippy::exhaustive_structs)] // TODO
        pub struct $name($value);

        impl TryFrom<$crate::extensions::unicode::Value> for $name {
            type Error = $crate::preferences::extensions::unicode::errors::PreferencesParseError;

            fn try_from(
                input: $crate::extensions::unicode::Value,
            ) -> Result<Self, Self::Error> {
                $try_from(input)
            }
        }

        impl From<$name> for $crate::extensions::unicode::Value {
            fn from(input: $name) -> $crate::extensions::unicode::Value {
                $into(input)
            }
        }

        impl $crate::preferences::PreferenceKey for $name {
            fn unicode_extension_key() -> Option<$crate::extensions::unicode::Key> {
                Some($crate::extensions::unicode::key!($ext_key))
            }

            fn try_from_key_value(
                key: &$crate::extensions::unicode::Key,
                value: &$crate::extensions::unicode::Value,
            ) -> Result<Option<Self>, $crate::preferences::extensions::unicode::errors::PreferencesParseError> {
                if Self::unicode_extension_key() == Some(*key) {
                    let result = Self::try_from(value.clone())?;
                    Ok(Some(result))
                } else {
                    Ok(None)
                }
            }

            fn unicode_extension_value(
                &self,
            ) -> Option<$crate::extensions::unicode::Value> {
                Some(self.clone().into())
            }
        }

        impl core::ops::Deref for $name {
            type Target = $value;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}
pub use __struct_keyword as struct_keyword;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        extensions::unicode,
        subtags::{subtag, Subtag},
    };
    use core::str::FromStr;

    #[test]
    fn struct_keywords_test() {
        struct_keyword!(
            DummyKeyword,
            "dk",
            Subtag,
            |input: unicode::Value| {
                if let Some(subtag) = input.into_single_subtag() {
                    if subtag.len() == 3 {
                        return Ok(DummyKeyword(subtag));
                    }
                }
                Err(crate::preferences::extensions::unicode::errors::PreferencesParseError::InvalidKeywordValue)
            },
            |input: DummyKeyword| { unicode::Value::from_subtag(Some(input.0)) }
        );

        let v = unicode::Value::from_str("foo").unwrap();
        let dk: DummyKeyword = v.clone().try_into().unwrap();
        assert_eq!(dk, DummyKeyword(subtag!("foo")));
        assert_eq!(unicode::Value::from(dk), v);

        let v = unicode::Value::from_str("foobar").unwrap();
        let dk: Result<DummyKeyword, _> = v.clone().try_into();
        assert!(dk.is_err());
    }
}
