use std::{
    borrow::{Borrow, Cow},
    fmt::{self, Debug, Display, Formatter},
    iter::repeat_with,
    ops::Deref,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{de, Deserialize, Serialize};
use static_assertions::assert_impl_all;
use zvariant::{Str, Type};

/// A D-Bus server GUID.
///
/// See the D-Bus specification [UUIDs chapter] for details.
///
/// You can create a `Guid` from an existing string with [`Guid::try_from::<&str>`][TryFrom].
///
/// [UUIDs chapter]: https://dbus.freedesktop.org/doc/dbus-specification.html#uuids
/// [TryFrom]: #impl-TryFrom%3C%26%27_%20str%3E
#[derive(Clone, Debug, PartialEq, Eq, Hash, Type, Serialize)]
pub struct Guid<'g>(Str<'g>);

assert_impl_all!(Guid<'_>: Send, Sync, Unpin);

impl Guid<'_> {
    /// Generate a D-Bus GUID that can be used with e.g.
    /// [`connection::Builder::server`](crate::connection::Builder::server).
    pub fn generate() -> Guid<'static> {
        let r: Vec<u32> = repeat_with(rand::random::<u32>).take(3).collect();
        let r3 = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(n) => n.as_secs() as u32,
            Err(_) => rand::random::<u32>(),
        };

        let s = format!("{:08x}{:08x}{:08x}{:08x}", r[0], r[1], r[2], r3);
        Guid(s.into())
    }

    /// Returns a string slice for the GUID.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Same as `try_from`, except it takes a `&'static str`.
    pub fn from_static_str(guid: &'static str) -> crate::Result<Self> {
        validate_guid(guid)?;

        Ok(Self(Str::from_static(guid)))
    }

    /// Create an owned copy of the GUID.
    pub fn to_owned(&self) -> Guid<'static> {
        Guid(self.0.to_owned())
    }
}

impl fmt::Display for Guid<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl<'g> TryFrom<&'g str> for Guid<'g> {
    type Error = crate::Error;

    /// Creates a GUID from a string with 32 hex digits.
    ///
    /// Returns `Err(`[`Error::InvalidGUID`]`)` if the provided string is not a well-formed GUID.
    ///
    /// [`Error::InvalidGUID`]: enum.Error.html#variant.InvalidGUID
    fn try_from(value: &'g str) -> std::result::Result<Self, Self::Error> {
        validate_guid(value)?;

        Ok(Self(Str::from(value)))
    }
}

impl<'g> TryFrom<Str<'g>> for Guid<'g> {
    type Error = crate::Error;

    /// Creates a GUID from a string with 32 hex digits.
    ///
    /// Returns `Err(`[`Error::InvalidGUID`]`)` if the provided string is not a well-formed GUID.
    ///
    /// [`Error::InvalidGUID`]: enum.Error.html#variant.InvalidGUID
    fn try_from(value: Str<'g>) -> std::result::Result<Self, Self::Error> {
        validate_guid(&value)?;

        Ok(Guid(value))
    }
}

impl TryFrom<String> for Guid<'_> {
    type Error = crate::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        validate_guid(&value)?;

        Ok(Guid(value.into()))
    }
}

impl<'g> TryFrom<Cow<'g, str>> for Guid<'g> {
    type Error = crate::Error;

    fn try_from(value: Cow<'g, str>) -> std::result::Result<Self, Self::Error> {
        validate_guid(&value)?;

        Ok(Guid(value.into()))
    }
}

impl FromStr for Guid<'static> {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.try_into().map(|guid: Guid<'_>| guid.to_owned())
    }
}

impl<'de> Deserialize<'de> for Guid<'de> {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <Cow<'de, str>>::deserialize(deserializer)
            .and_then(|s| s.try_into().map_err(serde::de::Error::custom))
    }
}

fn validate_guid(value: &str) -> crate::Result<()> {
    if value.as_bytes().len() != 32 || value.chars().any(|c| !char::is_ascii_hexdigit(&c)) {
        return Err(crate::Error::InvalidGUID);
    }

    Ok(())
}

impl From<Guid<'_>> for String {
    fn from(guid: Guid<'_>) -> Self {
        guid.0.into()
    }
}

impl Deref for Guid<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for Guid<'_> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Guid<'_> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

/// Owned version of [`Guid`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Type, Serialize)]
pub struct OwnedGuid(#[serde(borrow)] Guid<'static>);

assert_impl_all!(OwnedGuid: Send, Sync, Unpin);

impl OwnedGuid {
    /// Get a reference to the inner [`Guid`].
    pub fn inner(&self) -> &Guid<'static> {
        &self.0
    }
}

impl Deref for OwnedGuid {
    type Target = Guid<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for OwnedGuid {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl From<OwnedGuid> for Guid<'_> {
    fn from(o: OwnedGuid) -> Self {
        o.0
    }
}

impl<'unowned, 'owned: 'unowned> From<&'owned OwnedGuid> for Guid<'unowned> {
    fn from(guid: &'owned OwnedGuid) -> Self {
        guid.0.clone()
    }
}

impl From<Guid<'_>> for OwnedGuid {
    fn from(guid: Guid<'_>) -> Self {
        OwnedGuid(guid.to_owned())
    }
}

impl From<OwnedGuid> for Str<'_> {
    fn from(value: OwnedGuid) -> Self {
        value.0 .0
    }
}

impl<'de> Deserialize<'de> for OwnedGuid {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .and_then(|n| Guid::try_from(n).map_err(|e| de::Error::custom(e.to_string())))
            .map(Self)
    }
}

impl PartialEq<&str> for OwnedGuid {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<Guid<'_>> for OwnedGuid {
    fn eq(&self, other: &Guid<'_>) -> bool {
        self.0 == *other
    }
}

impl Display for OwnedGuid {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&Guid::from(self), f)
    }
}

#[cfg(test)]
mod tests {
    use crate::Guid;
    use test_log::test;

    #[test]
    fn generate() {
        let u1 = Guid::generate();
        let u2 = Guid::generate();
        assert_eq!(u1.as_str().len(), 32);
        assert_eq!(u2.as_str().len(), 32);
        assert_ne!(u1, u2);
        assert_ne!(u1.as_str(), u2.as_str());
    }
}
