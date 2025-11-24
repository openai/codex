//! Untagged serialization/deserialization support for Either<L, R>.
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
//!     #[serde(with = "either::serde_untagged")]
//!     inner: Either<Vec<String>, HashMap<String, i32>>
//! };
//!
//! // serialization
//! let data = IntOrString {
//!     inner: Either::Left(vec!["Hello".to_string()])
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

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum Either<L, R> {
    Left(L),
    Right(R),
}

pub fn serialize<L, R, S>(this: &super::Either<L, R>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    L: Serialize,
    R: Serialize,
{
    let untagged = match this {
        super::Either::Left(left) => Either::Left(left),
        super::Either::Right(right) => Either::Right(right),
    };
    untagged.serialize(serializer)
}

pub fn deserialize<'de, L, R, D>(deserializer: D) -> Result<super::Either<L, R>, D::Error>
where
    D: Deserializer<'de>,
    L: Deserialize<'de>,
    R: Deserialize<'de>,
{
    match Either::deserialize(deserializer) {
        Ok(Either::Left(left)) => Ok(super::Either::Left(left)),
        Ok(Either::Right(right)) => Ok(super::Either::Right(right)),
        Err(error) => Err(error),
    }
}
