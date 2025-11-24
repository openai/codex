use core::fmt::{self, Debug, Display, Formatter};
use serde::de::{Deserialize, Deserializer};
use static_assertions::assert_impl_all;

use crate::{Error, Result, Signature, Type};

/// [`Signature`] that identifies a complete type.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Type)]
pub struct CompleteType<'a>(Signature<'a>);

assert_impl_all!(CompleteType<'_>: Send, Sync, Unpin);

impl<'a> CompleteType<'a> {
    /// Returns the underlying [`Signature`]
    pub fn signature(&self) -> &Signature<'a> {
        &self.0
    }
}

impl<'a> Display for CompleteType<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        std::fmt::Display::fmt(&self.0.as_str(), f)
    }
}

impl<'a> TryFrom<Signature<'a>> for CompleteType<'a> {
    type Error = Error;

    fn try_from(sig: Signature<'a>) -> Result<Self> {
        if sig.n_complete_types() != Ok(1) {
            return Err(Error::IncorrectType);
        }
        Ok(Self(sig))
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for CompleteType<'a> {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = Signature::deserialize(deserializer)?;

        Self::try_from(val).map_err(serde::de::Error::custom)
    }
}
