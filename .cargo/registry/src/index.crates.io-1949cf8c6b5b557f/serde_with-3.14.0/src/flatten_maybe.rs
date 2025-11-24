/// Support deserializing from flattened and non-flattened representation
///
/// When working with different serialization formats, sometimes it is more idiomatic to flatten
/// fields, while other formats prefer nesting. Using `#[serde(flatten)]` only the flattened form
/// is supported.
///
/// This helper creates a function, which support deserializing from either the flattened or the
/// nested form. It gives an error, when both forms are provided. The `flatten` attribute is
/// required on the field such that the helper works. The serialization format will always be
/// flattened.
///
/// # Examples
///
/// ```rust
/// # use serde::Deserialize;
/// #
/// // Setup the types
/// #[derive(Deserialize, Debug)]
/// struct S {
///     #[serde(flatten, deserialize_with = "deserialize_t")]
///     t: T,
/// }
///
/// #[derive(Deserialize, Debug)]
/// struct T {
///     i: i32,
/// }
///
/// // The macro creates custom deserialization code.
/// // You need to specify a function name and the field name of the flattened field.
/// serde_with::flattened_maybe!(deserialize_t, "t");
///
/// # fn main() {
/// // Supports both flattened
/// let j = r#" {"i":1} "#;
/// assert!(serde_json::from_str::<S>(j).is_ok());
/// # // Ensure the t field is not dead code
/// # assert_eq!(serde_json::from_str::<S>(j).unwrap().t.i, 1);
///
/// // and non-flattened versions.
/// let j = r#" {"t":{"i":1}} "#;
/// assert!(serde_json::from_str::<S>(j).is_ok());
///
/// // Ensure that the value is given
/// let j = r#" {} "#;
/// assert!(serde_json::from_str::<S>(j).is_err());
///
/// // and only occurs once, not multiple times.
/// let j = r#" {"i":1,"t":{"i":1}} "#;
/// assert!(serde_json::from_str::<S>(j).is_err());
/// # }
/// ```
#[macro_export]
macro_rules! flattened_maybe {
    ($fn:ident, $field:tt) => {
        fn $fn<'de, T, D>(deserializer: D) -> $crate::__private__::Result<T, D::Error>
        where
            T: $crate::serde::Deserialize<'de>,
            D: $crate::serde::Deserializer<'de>,
        {
            use $crate::{
                __private__::{
                    Option::{self, None, Some},
                    Result::{self, Err, Ok},
                },
                serde,
            };

            #[derive($crate::serde_derive::Deserialize)]
            #[serde(crate = "serde")]
            pub struct Both<T> {
                #[serde(flatten)]
                flat: Option<T>,
                #[serde(rename = $field)]
                not_flat: Option<T>,
            }

            let both: Both<T> = $crate::serde::Deserialize::deserialize(deserializer)?;
            match (both.flat, both.not_flat) {
                (Some(t), None) | (None, Some(t)) => Ok(t),
                (None, None) => Err($crate::serde::de::Error::missing_field($field)),
                (Some(_), Some(_)) => Err($crate::serde::de::Error::custom(concat!(
                    "`",
                    $field,
                    "` is both flattened and not"
                ))),
            }
        }
    };
}
