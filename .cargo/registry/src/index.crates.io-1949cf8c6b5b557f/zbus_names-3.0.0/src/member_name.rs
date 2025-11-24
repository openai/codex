use crate::{
    utils::{impl_str_basic, impl_try_from},
    Error, Result,
};
use serde::{de, Deserialize, Serialize};
use static_assertions::assert_impl_all;
use std::{
    borrow::{Borrow, Cow},
    fmt::{self, Debug, Display, Formatter},
    ops::Deref,
    sync::Arc,
};
use zvariant::{NoneValue, OwnedValue, Str, Type, Value};

/// String that identifies an [member (method or signal) name][in] on the bus.
///
/// # Examples
///
/// ```
/// use zbus_names::MemberName;
///
/// // Valid member names.
/// let name = MemberName::try_from("Member_for_you").unwrap();
/// assert_eq!(name, "Member_for_you");
/// let name = MemberName::try_from("CamelCase101").unwrap();
/// assert_eq!(name, "CamelCase101");
/// let name = MemberName::try_from("a_very_loooooooooooooooooo_ooooooo_0000o0ngName").unwrap();
/// assert_eq!(name, "a_very_loooooooooooooooooo_ooooooo_0000o0ngName");
///
/// // Invalid member names
/// MemberName::try_from("").unwrap_err();
/// MemberName::try_from(".").unwrap_err();
/// MemberName::try_from("1startWith_a_Digit").unwrap_err();
/// MemberName::try_from("contains.dots_in_the_name").unwrap_err();
/// MemberName::try_from("contains-dashes-in_the_name").unwrap_err();
/// ```
///
/// [in]: https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-names-member
#[derive(
    Clone, Debug, Hash, PartialEq, Eq, Serialize, Type, Value, PartialOrd, Ord, OwnedValue,
)]
pub struct MemberName<'name>(Str<'name>);

assert_impl_all!(MemberName<'_>: Send, Sync, Unpin);

impl_str_basic!(MemberName<'_>);

impl<'name> MemberName<'name> {
    /// This is faster than `Clone::clone` when `self` contains owned data.
    pub fn as_ref(&self) -> MemberName<'_> {
        MemberName(self.0.as_ref())
    }

    /// The member name as string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Create a new `MemberName` from the given string.
    ///
    /// Since the passed string is not checked for correctness, prefer using the
    /// `TryFrom<&str>` implementation.
    pub fn from_str_unchecked(name: &'name str) -> Self {
        Self(Str::from(name))
    }

    /// Same as `try_from`, except it takes a `&'static str`.
    pub fn from_static_str(name: &'static str) -> Result<Self> {
        ensure_correct_member_name(name)?;
        Ok(Self(Str::from_static(name)))
    }

    /// Same as `from_str_unchecked`, except it takes a `&'static str`.
    pub const fn from_static_str_unchecked(name: &'static str) -> Self {
        Self(Str::from_static(name))
    }

    /// Same as `from_str_unchecked`, except it takes an owned `String`.
    ///
    /// Since the passed string is not checked for correctness, prefer using the
    /// `TryFrom<String>` implementation.
    pub fn from_string_unchecked(name: String) -> Self {
        Self(Str::from(name))
    }

    /// Creates an owned clone of `self`.
    pub fn to_owned(&self) -> MemberName<'static> {
        MemberName(self.0.to_owned())
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> MemberName<'static> {
        MemberName(self.0.into_owned())
    }
}

impl Deref for MemberName<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for MemberName<'_> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Display for MemberName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.as_str(), f)
    }
}

impl PartialEq<str> for MemberName<'_> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for MemberName<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<OwnedMemberName> for MemberName<'_> {
    fn eq(&self, other: &OwnedMemberName) -> bool {
        *self == other.0
    }
}

impl<'de: 'name, 'name> Deserialize<'de> for MemberName<'name> {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let name = <Cow<'name, str>>::deserialize(deserializer)?;

        Self::try_from(name).map_err(|e| de::Error::custom(e.to_string()))
    }
}

impl<'name> From<MemberName<'name>> for Str<'name> {
    fn from(value: MemberName<'name>) -> Self {
        value.0
    }
}

impl_try_from! {
    ty: MemberName<'s>,
    owned_ty: OwnedMemberName,
    validate_fn: ensure_correct_member_name,
    try_from: [&'s str, String, Arc<str>, Cow<'s, str>, Str<'s>],
}

fn ensure_correct_member_name(name: &str) -> Result<()> {
    // Rules
    //
    // * Only ASCII alphanumeric or `_`.
    // * Must not begin with a digit.
    // * Must contain at least 1 character.
    // * <= 255 characters.
    if name.is_empty() {
        return Err(Error::InvalidMemberName(format!(
            "`{}` is {} characters long, which is smaller than minimum allowed (1)",
            name,
            name.len(),
        )));
    } else if name.len() > 255 {
        return Err(Error::InvalidMemberName(format!(
            "`{}` is {} characters long, which is longer than maximum allowed (255)",
            name,
            name.len(),
        )));
    }

    // SAFETY: We established above that there is at least 1 character so unwrap is fine.
    if name.chars().next().unwrap().is_ascii_digit() {
        return Err(Error::InvalidMemberName(String::from(
            "must not start with a digit",
        )));
    }

    for c in name.chars() {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(Error::InvalidMemberName(format!(
                "`{c}` character not allowed"
            )));
        }
    }

    Ok(())
}

/// This never succeeds but is provided so it's easier to pass `Option::None` values for API
/// requiring `Option<TryInto<impl BusName>>`, since type inference won't work here.
impl TryFrom<()> for MemberName<'_> {
    type Error = Error;

    fn try_from(_value: ()) -> Result<Self> {
        unreachable!("Conversion from `()` is not meant to actually work");
    }
}

impl<'name> From<&MemberName<'name>> for MemberName<'name> {
    fn from(name: &MemberName<'name>) -> Self {
        name.clone()
    }
}

impl<'name> NoneValue for MemberName<'name> {
    type NoneType = &'name str;

    fn null_value() -> Self::NoneType {
        <&str>::default()
    }
}

/// Owned sibling of [`MemberName`].
#[derive(Clone, Hash, PartialEq, Eq, Serialize, Type, Value, PartialOrd, Ord, OwnedValue)]
pub struct OwnedMemberName(#[serde(borrow)] MemberName<'static>);

assert_impl_all!(OwnedMemberName: Send, Sync, Unpin);

impl_str_basic!(OwnedMemberName);

impl OwnedMemberName {
    /// Convert to the inner `MemberName`, consuming `self`.
    pub fn into_inner(self) -> MemberName<'static> {
        self.0
    }

    /// Get a reference to the inner `MemberName`.
    pub fn inner(&self) -> &MemberName<'static> {
        &self.0
    }
}

impl Deref for OwnedMemberName {
    type Target = MemberName<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for OwnedMemberName {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl From<OwnedMemberName> for MemberName<'_> {
    fn from(o: OwnedMemberName) -> Self {
        o.into_inner()
    }
}

impl<'unowned, 'owned: 'unowned> From<&'owned OwnedMemberName> for MemberName<'unowned> {
    fn from(name: &'owned OwnedMemberName) -> Self {
        MemberName::from_str_unchecked(name.as_str())
    }
}

impl From<MemberName<'_>> for OwnedMemberName {
    fn from(name: MemberName<'_>) -> Self {
        OwnedMemberName(name.into_owned())
    }
}

impl From<OwnedMemberName> for Str<'_> {
    fn from(value: OwnedMemberName) -> Self {
        value.into_inner().0
    }
}

impl<'de> Deserialize<'de> for OwnedMemberName {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .and_then(|n| MemberName::try_from(n).map_err(|e| de::Error::custom(e.to_string())))
            .map(Self)
    }
}

impl PartialEq<&str> for OwnedMemberName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<MemberName<'_>> for OwnedMemberName {
    fn eq(&self, other: &MemberName<'_>) -> bool {
        self.0 == *other
    }
}

impl Debug for OwnedMemberName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OwnedMemberName")
            .field(&self.as_str())
            .finish()
    }
}

impl Display for OwnedMemberName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&MemberName::from(self), f)
    }
}

impl NoneValue for OwnedMemberName {
    type NoneType = <MemberName<'static> as NoneValue>::NoneType;

    fn null_value() -> Self::NoneType {
        MemberName::null_value()
    }
}
