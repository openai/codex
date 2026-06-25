use std::collections::BTreeSet;
use std::fmt;
use std::num::NonZeroU32;

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::de::Error as _;

/// Correlates one client operation request with the host's response.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RequestId(i64);

impl RequestId {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }
}

/// Correlates one host delegate request with the client's response.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DelegateRequestId(i64);

impl DelegateRequestId {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ProtocolVersion(NonZeroU32);

impl ProtocolVersion {
    pub const V1: Self = Self(NonZeroU32::MIN);

    pub const fn new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidIdentifier;

impl fmt::Display for InvalidIdentifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("identifier must not be empty")
    }
}

impl std::error::Error for InvalidIdentifier {}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct NonEmptyString(String);

impl NonEmptyString {
    fn new(value: impl Into<String>) -> Result<Self, InvalidIdentifier> {
        let value = value.into();
        if value.trim().is_empty() {
            Err(InvalidIdentifier)
        } else {
            Ok(Self(value))
        }
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for NonEmptyString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NonEmptyString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// A named protocol feature advertised during connection negotiation.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Capability(NonEmptyString);

impl Capability {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidIdentifier> {
        NonEmptyString::new(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Identifies one logical code-mode session on a connection.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SessionId(NonEmptyString);

impl SessionId {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidIdentifier> {
        NonEmptyString::new(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CapabilitySet(BTreeSet<Capability>);

impl CapabilitySet {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn try_new(
        capabilities: impl IntoIterator<Item = Capability>,
    ) -> Result<Self, DuplicateCapability> {
        let mut unique = BTreeSet::new();
        for capability in capabilities {
            if !unique.insert(capability.clone()) {
                return Err(DuplicateCapability { capability });
            }
        }
        Ok(Self(unique))
    }

    pub fn contains(&self, capability: &Capability) -> bool {
        self.0.contains(capability)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.0.iter()
    }
}

impl<'de> Deserialize<'de> for CapabilitySet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(Vec::<Capability>::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DuplicateCapability {
    capability: Capability,
}

impl fmt::Display for DuplicateCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "duplicate capability `{}`", self.capability)
    }
}

impl std::error::Error for DuplicateCapability {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SupportedProtocolVersions(BTreeSet<ProtocolVersion>);

impl SupportedProtocolVersions {
    pub fn try_new(
        versions: impl IntoIterator<Item = ProtocolVersion>,
    ) -> Result<Self, InvalidSupportedProtocolVersions> {
        let mut unique = BTreeSet::new();
        for version in versions {
            if !unique.insert(version) {
                return Err(InvalidSupportedProtocolVersions::Duplicate(version));
            }
        }
        if unique.is_empty() {
            return Err(InvalidSupportedProtocolVersions::Empty);
        }
        Ok(Self(unique))
    }

    pub fn contains(&self, version: ProtocolVersion) -> bool {
        self.0.contains(&version)
    }

    pub fn iter(&self) -> impl Iterator<Item = ProtocolVersion> + '_ {
        self.0.iter().copied()
    }
}

impl<'de> Deserialize<'de> for SupportedProtocolVersions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(Vec::<ProtocolVersion>::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidSupportedProtocolVersions {
    Empty,
    Duplicate(ProtocolVersion),
}

impl fmt::Display for InvalidSupportedProtocolVersions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("at least one protocol version is required"),
            Self::Duplicate(version) => {
                write!(formatter, "duplicate protocol version {}", version.get())
            }
        }
    }
}

impl std::error::Error for InvalidSupportedProtocolVersions {}
