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

/// String that identifies a [unique bus name][ubn].
///
/// # Examples
///
/// ```
/// use zbus_names::UniqueName;
///
/// // Valid unique names.
/// let name = UniqueName::try_from(":org.gnome.Service-for_you").unwrap();
/// assert_eq!(name, ":org.gnome.Service-for_you");
/// let name = UniqueName::try_from(":a.very.loooooooooooooooooo-ooooooo_0000o0ng.Name").unwrap();
/// assert_eq!(name, ":a.very.loooooooooooooooooo-ooooooo_0000o0ng.Name");
///
/// // Invalid unique names
/// UniqueName::try_from("").unwrap_err();
/// UniqueName::try_from("dont.start.with.a.colon").unwrap_err();
/// UniqueName::try_from(":double..dots").unwrap_err();
/// UniqueName::try_from(".").unwrap_err();
/// UniqueName::try_from(".start.with.dot").unwrap_err();
/// UniqueName::try_from(":no-dots").unwrap_err();
/// ```
///
/// [ubn]: https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-names-bus
#[derive(
    Clone, Debug, Hash, PartialEq, Eq, Serialize, Type, Value, PartialOrd, Ord, OwnedValue,
)]
pub struct UniqueName<'name>(Str<'name>);

assert_impl_all!(UniqueName<'_>: Send, Sync, Unpin);

impl_str_basic!(UniqueName<'_>);

impl<'name> UniqueName<'name> {
    /// This is faster than `Clone::clone` when `self` contains owned data.
    pub fn as_ref(&self) -> UniqueName<'_> {
        UniqueName(self.0.as_ref())
    }

    /// The unique name as string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Create a new `UniqueName` from the given string.
    ///
    /// Since the passed string is not checked for correctness, prefer using the
    /// `TryFrom<&str>` implementation.
    pub fn from_str_unchecked(name: &'name str) -> Self {
        Self(Str::from(name))
    }

    /// Same as `try_from`, except it takes a `&'static str`.
    pub fn from_static_str(name: &'static str) -> Result<Self> {
        ensure_correct_unique_name(name)?;
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
    pub fn to_owned(&self) -> UniqueName<'static> {
        UniqueName(self.0.to_owned())
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> UniqueName<'static> {
        UniqueName(self.0.into_owned())
    }
}

impl Deref for UniqueName<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for UniqueName<'_> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Display for UniqueName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.as_str(), f)
    }
}

impl PartialEq<str> for UniqueName<'_> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for UniqueName<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<OwnedUniqueName> for UniqueName<'_> {
    fn eq(&self, other: &OwnedUniqueName) -> bool {
        *self == other.0
    }
}

impl<'de: 'name, 'name> Deserialize<'de> for UniqueName<'name> {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let name = <Cow<'name, str>>::deserialize(deserializer)?;

        Self::try_from(name).map_err(|e| de::Error::custom(e.to_string()))
    }
}

fn ensure_correct_unique_name(name: &str) -> Result<()> {
    // Rules
    //
    // * Only ASCII alphanumeric, `_` or '-'
    // * Must begin with a `:`.
    // * Must contain at least one `.`.
    // * <= 255 characters.
    if name.is_empty() {
        return Err(Error::InvalidUniqueName(String::from(
            "must contain at least 4 characters",
        )));
    } else if name.len() > 255 {
        return Err(Error::InvalidUniqueName(format!(
            "`{}` is {} characters long, which is longer than maximum allowed (255)",
            name,
            name.len(),
        )));
    } else if name == "org.freedesktop.DBus" {
        // Bus itself uses its well-known name as its unique name.
        return Ok(());
    }

    // SAFETY: Just checked above that we've at least 1 character.
    let mut chars = name.chars();
    let mut prev = match chars.next().expect("no first char") {
        first @ ':' => first,
        _ => {
            return Err(Error::InvalidUniqueName(String::from(
                "must start with a `:`",
            )));
        }
    };

    let mut no_dot = true;
    for c in chars {
        if c == '.' {
            if prev == '.' {
                return Err(Error::InvalidUniqueName(String::from(
                    "must not contain a double `.`",
                )));
            }

            if no_dot {
                no_dot = false;
            }
        } else if !c.is_ascii_alphanumeric() && c != '_' && c != '-' {
            return Err(Error::InvalidUniqueName(format!(
                "`{c}` character not allowed"
            )));
        }

        prev = c;
    }

    if no_dot {
        return Err(Error::InvalidUniqueName(String::from(
            "must contain at least 1 `.`",
        )));
    }

    Ok(())
}

/// This never succeeds but is provided so it's easier to pass `Option::None` values for API
/// requiring `Option<TryInto<impl BusName>>`, since type inference won't work here.
impl TryFrom<()> for UniqueName<'_> {
    type Error = Error;

    fn try_from(_value: ()) -> Result<Self> {
        unreachable!("Conversion from `()` is not meant to actually work");
    }
}

impl<'name> From<&UniqueName<'name>> for UniqueName<'name> {
    fn from(name: &UniqueName<'name>) -> Self {
        name.clone()
    }
}

impl<'name> From<UniqueName<'name>> for Str<'name> {
    fn from(value: UniqueName<'name>) -> Self {
        value.0
    }
}

impl<'name> NoneValue for UniqueName<'name> {
    type NoneType = &'name str;

    fn null_value() -> Self::NoneType {
        <&str>::default()
    }
}

/// Owned sibling of [`UniqueName`].
#[derive(Clone, Hash, PartialEq, Eq, Serialize, Type, Value, PartialOrd, Ord, OwnedValue)]
pub struct OwnedUniqueName(#[serde(borrow)] UniqueName<'static>);

assert_impl_all!(OwnedUniqueName: Send, Sync, Unpin);

impl_str_basic!(OwnedUniqueName);

impl OwnedUniqueName {
    /// Convert to the inner `UniqueName`, consuming `self`.
    pub fn into_inner(self) -> UniqueName<'static> {
        self.0
    }

    /// Get a reference to the inner `UniqueName`.
    pub fn inner(&self) -> &UniqueName<'static> {
        &self.0
    }
}

impl Deref for OwnedUniqueName {
    type Target = UniqueName<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for OwnedUniqueName {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl From<OwnedUniqueName> for UniqueName<'_> {
    fn from(o: OwnedUniqueName) -> Self {
        o.into_inner()
    }
}

impl<'unowned, 'owned: 'unowned> From<&'owned OwnedUniqueName> for UniqueName<'unowned> {
    fn from(name: &'owned OwnedUniqueName) -> Self {
        UniqueName::from_str_unchecked(name.as_str())
    }
}

impl From<UniqueName<'_>> for OwnedUniqueName {
    fn from(name: UniqueName<'_>) -> Self {
        OwnedUniqueName(name.into_owned())
    }
}

impl_try_from! {
    ty: UniqueName<'s>,
    owned_ty: OwnedUniqueName,
    validate_fn: ensure_correct_unique_name,
    try_from: [&'s str, String, Arc<str>, Cow<'s, str>, Str<'s>],
}

impl From<OwnedUniqueName> for Str<'_> {
    fn from(value: OwnedUniqueName) -> Self {
        value.into_inner().0
    }
}

impl<'de> Deserialize<'de> for OwnedUniqueName {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .and_then(|n| UniqueName::try_from(n).map_err(|e| de::Error::custom(e.to_string())))
            .map(Self)
    }
}

impl PartialEq<&str> for OwnedUniqueName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<UniqueName<'_>> for OwnedUniqueName {
    fn eq(&self, other: &UniqueName<'_>) -> bool {
        self.0 == *other
    }
}

impl NoneValue for OwnedUniqueName {
    type NoneType = <UniqueName<'static> as NoneValue>::NoneType;

    fn null_value() -> Self::NoneType {
        UniqueName::null_value()
    }
}

impl Debug for OwnedUniqueName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OwnedUniqueName")
            .field(&self.as_str())
            .finish()
    }
}

impl Display for OwnedUniqueName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&UniqueName::from(self), f)
    }
}
