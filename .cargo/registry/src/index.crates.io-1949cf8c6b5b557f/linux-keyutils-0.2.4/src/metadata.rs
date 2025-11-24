use crate::ffi::{self, KeyCtlOperation, KeySerialId};
use crate::utils::{CStr, String};
use crate::{KeyError, KeyPermissions, KeyType};
use alloc::string::ToString;
use core::str::{self, FromStr};

/// Information about the given node/entry.
/// Returned by [Key::metadata](crate::Key::metadata)
/// or [KeyRing::metadata](crate::KeyRing::metadata)
#[derive(Debug, Clone)]
pub struct Metadata {
    ktype: KeyType,
    uid: u32,
    gid: u32,
    perm: KeyPermissions,
    description: String,
}

impl FromStr for Metadata {
    type Err = KeyError;

    /// The returned string contains the following information about
    /// the key:
    ///
    /// `type;uid;gid;perm;description`
    ///
    /// And is then parsed into a valid [Metadata] struct.
    ///
    /// In the above, type and description are strings, uid and gid are
    /// decimal strings, and perm is a hexadecimal permissions mask.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Begin parsing
        let mut iter = s.split(';');

        // Parse type into KeyType
        let ktype: KeyType = iter
            .next()
            .and_then(|v| v.try_into().ok())
            .ok_or(KeyError::InvalidDescription)?;

        // Parse the UID
        let uid = iter
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or(KeyError::InvalidDescription)?;

        // Parse the GID
        let gid = iter
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or(KeyError::InvalidDescription)?;

        // Parse the permissions
        let perms: u32 = iter
            .next()
            .and_then(|v| u32::from_str_radix(v, 16).ok())
            .ok_or(KeyError::InvalidDescription)?;

        // Copy the actual description
        let description = iter.next().ok_or(KeyError::InvalidDescription)?.to_string();

        // Create the description
        Ok(Self {
            ktype,
            uid,
            gid,
            perm: KeyPermissions::from_u32(perms),
            description,
        })
    }
}

impl Metadata {
    /// Internal method to derive information from an
    /// arbitrary node based on ID alone.
    pub(crate) fn from_id(id: KeySerialId) -> Result<Self, KeyError> {
        let mut result = alloc::vec![0u8; 512];

        // Obtain the description from the kernel
        let len = ffi::keyctl!(
            KeyCtlOperation::Describe,
            id.as_raw_id() as libc::c_ulong,
            result.as_mut_ptr() as _,
            result.len() as _
        )? as usize;

        // Construct the CStr first to remove the null terminator
        let cs = CStr::from_bytes_with_nul(&result[..len]).or(Err(KeyError::InvalidDescription))?;

        // Construct the string from the resulting data ensuring utf8 compat
        let s = cs.to_str().or(Err(KeyError::InvalidDescription))?;
        Self::from_str(s)
    }

    /// The type of this entry
    pub fn get_type(&self) -> KeyType {
        self.ktype
    }

    /// The owning UID of this entry
    pub fn get_uid(&self) -> u32 {
        self.uid
    }

    /// The owning GID of this entry
    pub fn get_gid(&self) -> u32 {
        self.gid
    }

    /// The current permissions of this entry
    pub fn get_perms(&self) -> KeyPermissions {
        self.perm
    }

    /// The description for this entry
    pub fn get_description(&self) -> &str {
        &self.description
    }
}
