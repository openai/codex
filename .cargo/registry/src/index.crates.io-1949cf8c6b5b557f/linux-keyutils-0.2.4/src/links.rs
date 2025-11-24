//! Helper types for iterating over keyring entries
//!
use crate::utils::Vec;
use crate::{Key, KeyError, KeyRing, KeySerialId, KeyType, Metadata};
use core::cmp::PartialEq;
use core::ops::Deref;

/// An item/node linked to a ring. Both keys and other keyrings
/// can be linked to a particular keyring.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LinkNode {
    KeyRing(KeyRing),
    Key(Key),
}

/// A collection of LinkNodes, returned from [KeyRing::get_links]
///
/// For example:
///
/// ```
/// use linux_keyutils::{Key, KeyRing, KeyRingIdentifier, KeyError};
///
/// // Test if a particular Key is linked to the user session KeyRing
/// fn is_linked_to_user_session(key: &Key) -> Result<bool, KeyError> {
///     // Get the  keyring
///     let ring = KeyRing::from_special_id(KeyRingIdentifier::UserSession, false)?;
///
///     // Locate all the links
///     let links = ring.get_links(200)?;
///
///     // Determine if the key is linked to the ring
///     Ok(links.contains(key))
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Links(Vec<LinkNode>);

impl PartialEq<Key> for LinkNode {
    fn eq(&self, other: &Key) -> bool {
        matches!(self, LinkNode::Key(x) if x == other)
    }
}

impl PartialEq<Key> for &LinkNode {
    fn eq(&self, other: &Key) -> bool {
        matches!(self, LinkNode::Key(x) if x == other)
    }
}

impl PartialEq<KeyRing> for LinkNode {
    fn eq(&self, other: &KeyRing) -> bool {
        matches!(self, LinkNode::KeyRing(x) if x == other)
    }
}

impl PartialEq<KeyRing> for &LinkNode {
    fn eq(&self, other: &KeyRing) -> bool {
        matches!(self, LinkNode::KeyRing(x) if x == other)
    }
}

impl LinkNode {
    /// Internal method to construct a LinkNode from a raw ID
    pub(crate) fn from_id(id: KeySerialId) -> Result<Self, KeyError> {
        let metadata = Metadata::from_id(id)?;
        let node = match metadata.get_type() {
            KeyType::KeyRing => Self::KeyRing(KeyRing::from_id(id)),
            KeyType::User => Self::Key(Key::from_id(id)),
            _ => return Err(KeyError::OperationNotSupported),
        };
        Ok(node)
    }

    /// Attempt to convert this LinkNode to a Key
    ///
    /// Returns the key if the entry is a Key, None otherwise.
    pub fn as_key(&self) -> Option<Key> {
        match self {
            Self::Key(inner) => Some(*inner),
            _ => None,
        }
    }

    /// Attempt to convert this LinkNode to a KeyRing
    ///
    /// Returns the ring if the entry is a KeyRing, None otherwise.
    pub fn as_ring(&self) -> Option<KeyRing> {
        match self {
            Self::KeyRing(inner) => Some(*inner),
            _ => None,
        }
    }
}

impl Deref for Links {
    type Target = Vec<LinkNode>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromIterator<LinkNode> for Links {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = LinkNode>,
    {
        Self(iter.into_iter().collect())
    }
}

impl Links {
    /// Internal constructor to abstract the list of objects
    pub fn new(inner: Vec<LinkNode>) -> Self {
        Self(inner)
    }

    /// Obtain the entry with the provided ID/Key/Keyring
    pub fn get<T>(&self, entry: &T) -> Option<&LinkNode>
    where
        LinkNode: PartialEq<T>,
    {
        self.0.iter().find(|v| *v == entry)
    }

    /// Check if the list contains the provided ID/Key/Keyring
    pub fn contains<T>(&self, entry: &T) -> bool
    where
        LinkNode: PartialEq<T>,
    {
        self.0.iter().any(|v| v == entry)
    }
}
