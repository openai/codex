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

/// String that identifies a [property][pn] name on the bus.
///
/// # Examples
///
/// ```
/// use zbus_names::PropertyName;
///
/// // Valid property names.
/// let name = PropertyName::try_from("Property_for_you").unwrap();
/// assert_eq!(name, "Property_for_you");
/// let name = PropertyName::try_from("CamelCase101").unwrap();
/// assert_eq!(name, "CamelCase101");
/// let name = PropertyName::try_from("a_very_loooooooooooooooooo_ooooooo_0000o0ngName").unwrap();
/// assert_eq!(name, "a_very_loooooooooooooooooo_ooooooo_0000o0ngName");
/// let name = PropertyName::try_from("Property_for_you-1").unwrap();
/// assert_eq!(name, "Property_for_you-1");
/// ```
///
/// [pn]: https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-properties
#[derive(
    Clone, Debug, Hash, PartialEq, Eq, Serialize, Type, Value, PartialOrd, Ord, OwnedValue,
)]
pub struct PropertyName<'name>(Str<'name>);

assert_impl_all!(PropertyName<'_>: Send, Sync, Unpin);

impl_str_basic!(PropertyName<'_>);

impl<'name> PropertyName<'name> {
    /// This is faster than `Clone::clone` when `self` contains owned data.
    pub fn as_ref(&self) -> PropertyName<'_> {
        PropertyName(self.0.as_ref())
    }

    /// The property name as string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Create a new `PropertyName` from the given string.
    ///
    /// Since the passed string is not checked for correctness, prefer using the
    /// `TryFrom<&str>` implementation.
    pub fn from_str_unchecked(name: &'name str) -> Self {
        Self(Str::from(name))
    }

    /// Same as `try_from`, except it takes a `&'static str`.
    pub fn from_static_str(name: &'static str) -> Result<Self> {
        ensure_correct_property_name(name)?;
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
    pub fn to_owned(&self) -> PropertyName<'static> {
        PropertyName(self.0.to_owned())
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> PropertyName<'static> {
        PropertyName(self.0.into_owned())
    }
}

impl Deref for PropertyName<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for PropertyName<'_> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Display for PropertyName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.as_str(), f)
    }
}

impl PartialEq<str> for PropertyName<'_> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for PropertyName<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<OwnedPropertyName> for PropertyName<'_> {
    fn eq(&self, other: &OwnedPropertyName) -> bool {
        *self == other.0
    }
}

impl<'de: 'name, 'name> Deserialize<'de> for PropertyName<'name> {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let name = <Cow<'name, str>>::deserialize(deserializer)?;

        Self::try_from(name).map_err(|e| de::Error::custom(e.to_string()))
    }
}

impl<'name> From<PropertyName<'name>> for Str<'name> {
    fn from(value: PropertyName<'name>) -> Self {
        value.0
    }
}

impl_try_from! {
  ty: PropertyName<'s>,
  owned_ty: OwnedPropertyName,
  validate_fn: ensure_correct_property_name,
  try_from: [&'s str, String, Arc<str>, Cow<'s, str>, Str<'s>],
}

fn ensure_correct_property_name(name: &str) -> Result<()> {
    // Rules
    //
    // * Only ASCII alphanumeric or `_`.
    // * Must not begin with a digit.
    // * Must contain at least 1 character.
    // * <= 255 characters.
    if name.is_empty() {
        return Err(Error::InvalidPropertyName(format!(
            "`{}` is {} characters long, which is smaller than minimum allowed (1)",
            name,
            name.len(),
        )));
    } else if name.len() > 255 {
        return Err(Error::InvalidPropertyName(format!(
            "`{}` is {} characters long, which is longer than maximum allowed (255)",
            name,
            name.len(),
        )));
    }

    Ok(())
}

/// This never succeeds but is provided so it's easier to pass `Option::None` values for API
/// requiring `Option<TryInto<impl BusName>>`, since type inference won't work here.
impl TryFrom<()> for PropertyName<'_> {
    type Error = Error;

    fn try_from(_value: ()) -> Result<Self> {
        unreachable!("Conversion from `()` is not meant to actually work");
    }
}

impl<'name> From<&PropertyName<'name>> for PropertyName<'name> {
    fn from(name: &PropertyName<'name>) -> Self {
        name.clone()
    }
}

impl<'name> NoneValue for PropertyName<'name> {
    type NoneType = &'name str;

    fn null_value() -> Self::NoneType {
        <&str>::default()
    }
}

/// Owned sibling of [`PropertyName`].
#[derive(Clone, Hash, PartialEq, Eq, Serialize, Type, Value, PartialOrd, Ord, OwnedValue)]
pub struct OwnedPropertyName(#[serde(borrow)] PropertyName<'static>);

assert_impl_all!(OwnedPropertyName: Send, Sync, Unpin);

impl_str_basic!(OwnedPropertyName);

impl OwnedPropertyName {
    /// Convert to the inner `PropertyName`, consuming `self`.
    pub fn into_inner(self) -> PropertyName<'static> {
        self.0
    }

    /// Get a reference to the inner `PropertyName`.
    pub fn inner(&self) -> &PropertyName<'static> {
        &self.0
    }
}

impl Deref for OwnedPropertyName {
    type Target = PropertyName<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for OwnedPropertyName {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl From<OwnedPropertyName> for PropertyName<'_> {
    fn from(o: OwnedPropertyName) -> Self {
        o.into_inner()
    }
}

impl<'unowned, 'owned: 'unowned> From<&'owned OwnedPropertyName> for PropertyName<'unowned> {
    fn from(name: &'owned OwnedPropertyName) -> Self {
        PropertyName::from_str_unchecked(name.as_str())
    }
}

impl From<PropertyName<'_>> for OwnedPropertyName {
    fn from(name: PropertyName<'_>) -> Self {
        OwnedPropertyName(name.into_owned())
    }
}

impl From<OwnedPropertyName> for Str<'_> {
    fn from(value: OwnedPropertyName) -> Self {
        value.into_inner().0
    }
}

impl<'de> Deserialize<'de> for OwnedPropertyName {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .and_then(|n| PropertyName::try_from(n).map_err(|e| de::Error::custom(e.to_string())))
            .map(Self)
    }
}

impl PartialEq<&str> for OwnedPropertyName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<PropertyName<'_>> for OwnedPropertyName {
    fn eq(&self, other: &PropertyName<'_>) -> bool {
        self.0 == *other
    }
}

impl Debug for OwnedPropertyName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OwnedPropertyName")
            .field(&self.as_str())
            .finish()
    }
}

impl Display for OwnedPropertyName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&PropertyName::from(self), f)
    }
}

impl NoneValue for OwnedPropertyName {
    type NoneType = <PropertyName<'static> as NoneValue>::NoneType;

    fn null_value() -> Self::NoneType {
        PropertyName::null_value()
    }
}
