//! Untagged serialization/deserialization support for Option<Either<L, R>>.
//!
//! `Either` uses default, externally-tagged representation.
//! However, sometimes it is useful to support several alternative types.
//! For example, we may have a field which is generally Map<String, i32>
//! but in typical cases Vec<String> would suffice, too.
//!
//! ```rust
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use either::Either;
//! use std::collections::HashMap;
//!
//! #[derive(serde::Serialize, serde::Deserialize, Debug)]
//! #[serde(transparent)]
//! struct IntOrString {
//!     #[serde(with = "either::serde_untagged_optional")]
//!     inner: Option<Either<Vec<String>, HashMap<String, i32>>>
//! };
//!
//! // serialization
//! let data = IntOrString {
//!     inner: Some(Either::Left(vec!["Hello".to_string()]))
//! };
//! // notice: no tags are emitted.
//! assert_eq!(serde_json::to_string(&data)?, r#"["Hello"]"#);
//!
//! // deserialization
//! let data: IntOrString = serde_json::from_str(
//!     r#"{"a": 0, "b": 14}"#
//! )?;
//! println!("found {:?}", data);
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Either<L, R> {
    Left(L),
    Right(R),
}

pub fn serialize<L, R, S>(
    this: &Option<super::Either<L, R>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    L: Serialize,
    R: Serialize,
{
    let untagged = match this {
        Some(super::Either::Left(left)) => Some(Either::Left(left)),
        Some(super::Either::Right(right)) => Some(Either::Right(right)),
        None => None,
    };
    untagged.serialize(serializer)
}

pub fn deserialize<'de, L, R, D>(deserializer: D) -> Result<Option<super::Either<L, R>>, D::Error>
where
    D: Deserializer<'de>,
    L: Deserialize<'de>,
    R: Deserialize<'de>,
{
    match Option::deserialize(deserializer) {
        Ok(Some(Either::Left(left))) => Ok(Some(super::Either::Left(left))),
        Ok(Some(Either::Right(right))) => Ok(Some(super::Either::Right(right))),
        Ok(None) => Ok(None),
        Err(error) => Err(error),
    }
}
