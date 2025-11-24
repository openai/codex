use crate::ffi::{self, KeyCtlOperation, KeySerialId};
use crate::utils::Vec;
use crate::{KeyError, KeyPermissions, Metadata};
use core::fmt;

/// A key corresponding to a specific real ID.
///
/// Generally you will either create or obtain a Key via the [KeyRing](crate::KeyRing)
/// interface. Since keys must be linked with a keyring to be valid.
///
/// For example:
///
/// ```
/// use linux_keyutils::{Key, KeyRing, KeyRingIdentifier, KeyError};
/// use zeroize::Zeroize;
///
/// // Name of my program's key
/// const KEYNAME: &'static str = "my-process-key";
///
/// // Locate the key in the process keyring and update the secret
/// fn update_secret<T: AsRef<[u8]> + Zeroize>(data: &T) -> Result<(), KeyError> {
///     // Get the current process keyring
///     let ring = KeyRing::from_special_id(KeyRingIdentifier::Process, false)?;
///
///     // Locate the key we previously created
///     let key = ring.search(KEYNAME)?;
///
///     // Change the data it contains
///     key.update(data)?;
///     Ok(())
/// }
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Key(KeySerialId);

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let info = self.metadata().map_err(|_| fmt::Error)?;
        write!(f, "Key({:?})", info)
    }
}

impl Key {
    /// Initialize a new [Key] object from the provided ID
    pub fn from_id(id: KeySerialId) -> Self {
        Self(id)
    }

    /// Obtain a copy of the ID of this key
    pub fn get_id(&self) -> KeySerialId {
        self.0
    }

    /// Obtain information describing the attributes of this key.
    ///
    /// The key must grant the caller view permission.
    pub fn metadata(&self) -> Result<Metadata, KeyError> {
        Metadata::from_id(self.0)
    }

    /// Read the payload data of a key into a provided mutable slice.
    ///
    /// The returned usize is the number of bytes read into the slice.
    ///
    /// The key must either grant the caller read permission, or grant
    /// the caller search permission when searched for from the process
    /// keyrings (i.e., the key is possessed).
    pub fn read<T: AsMut<[u8]>>(&self, buffer: &mut T) -> Result<usize, KeyError> {
        // TODO: alternate key types? Currenlty we don't support KeyType::BigKey
        let len = ffi::keyctl!(
            KeyCtlOperation::Read,
            self.0.as_raw_id() as libc::c_ulong,
            buffer.as_mut().as_mut_ptr() as _,
            buffer.as_mut().len() as _
        )? as usize;
        Ok(len)
    }

    /// Read the payload data of a key, returning a newly allocated vector.
    ///
    /// The key must either grant the caller read permission, or grant
    /// the caller search permission when searched for from the process
    /// keyrings (i.e., the key is possessed).
    pub fn read_to_vec(&self) -> Result<Vec<u8>, KeyError> {
        // Ensure we have enough room to write up to the maximum for a UserKey
        let mut buffer = Vec::with_capacity(65536);

        // Obtain the key
        let len = ffi::keyctl!(
            KeyCtlOperation::Read,
            self.0.as_raw_id() as libc::c_ulong,
            buffer.as_mut_ptr() as _,
            buffer.capacity() as _
        )? as usize;

        // Update length
        unsafe {
            buffer.set_len(len);
        }
        Ok(buffer)
    }

    /// Update a key's data payload.
    ///
    /// The caller must have write permission on the key specified and the key
    /// type must support updating.
    ///
    /// A negatively instantiated key (see the description of [Key::reject])
    /// can be positively instantiated with this operation.
    pub fn update<T: AsRef<[u8]>>(&self, update: &T) -> Result<(), KeyError> {
        _ = ffi::keyctl!(
            KeyCtlOperation::Update,
            self.0.as_raw_id() as libc::c_ulong,
            update.as_ref().as_ptr() as _,
            update.as_ref().len() as _
        )?;
        Ok(())
    }

    /// Change the permissions of the key with the ID provided
    ///
    /// If the caller doesn't have the CAP_SYS_ADMIN capability, it can change
    /// permissions only only for the keys it owns. (More precisely: the caller's
    /// filesystem UID must match the UID of the key.)
    pub fn set_perms(&self, perm: KeyPermissions) -> Result<(), KeyError> {
        _ = ffi::keyctl!(
            KeyCtlOperation::SetPerm,
            self.0.as_raw_id() as libc::c_ulong,
            perm.bits() as _
        )?;
        Ok(())
    }

    /// Change the ownership (user and group ID) of a key.
    ///
    /// For the UID to be changed, or for the GID to be changed to a group
    /// the caller is not a member of, the caller must have the CAP_SYS_ADMIN
    /// capability (see capabilities(7)).
    ///
    /// If the UID is to be changed, the new user must have sufficient quota
    /// to accept the key. The quota deduction will be removed from the old
    /// user to the new user should the UID be changed.
    pub fn chown(&self, uid: Option<u32>, gid: Option<u32>) -> Result<(), KeyError> {
        let uid_opt = uid.unwrap_or(u32::MAX);
        let gid_opt = gid.unwrap_or(u32::MAX);
        _ = ffi::keyctl!(
            KeyCtlOperation::Chown,
            self.0.as_raw_id() as libc::c_ulong,
            uid_opt as _,
            gid_opt as _
        )?;
        Ok(())
    }

    /// Set a timeout on a key.
    ///
    /// Specifying the timeout value as 0 clears any existing timeout on the key.
    ///
    /// The `/proc/keys` file displays the remaining time until each key will expire.
    /// (This is the only method of discovering the timeout on a key.)
    ///
    /// The caller must either have the setattr permission on the key or hold an
    /// instantiation authorization token for the key.
    ///
    /// The key and any links to the key will be automatically garbage collected
    /// after the  timeout  expires. Subsequent attempts to access the key will
    /// then fail with the error EKEYEXPIRED.
    ///
    /// This operation cannot be used to set timeouts on revoked, expired, or
    /// negatively instantiated keys.
    pub fn set_timeout(&self, seconds: usize) -> Result<(), KeyError> {
        _ = ffi::keyctl!(
            KeyCtlOperation::SetTimeout,
            self.0.as_raw_id() as libc::c_ulong,
            seconds as _
        )?;
        Ok(())
    }

    /// Revoke this key. Similar to [Key::reject] just without the timeout.
    ///
    /// The key is scheduled for garbage collection; it will no longer be findable,
    /// and will be unavailable for further operations. Further attempts to use the
    /// key will fail with the error `EKEYREVOKED`.
    ///
    /// The caller must have write or setattr permission on the key.
    pub fn revoke(&self) -> Result<(), KeyError> {
        _ = ffi::keyctl!(KeyCtlOperation::Revoke, self.0.as_raw_id() as libc::c_ulong)?;
        Ok(())
    }

    /// Mark a key as negatively instantiated and set an expiration timer on the key.
    ///
    /// This will prevent others from retrieving the key in further searches. And they
    /// will receive a `EKEYREJECTED` error when performing the search.
    ///
    /// Similar to [Key::revoke] but with a timeout.
    pub fn reject(&self, seconds: usize) -> Result<(), KeyError> {
        _ = ffi::keyctl!(
            KeyCtlOperation::Reject,
            self.0.as_raw_id() as libc::c_ulong,
            seconds as _,
            libc::EKEYREJECTED as _
        )?;
        Ok(())
    }

    /// Mark a key as invalid.
    ///
    /// To invalidate a key, the caller must have search permission on the
    /// key.
    ///
    /// This operation marks the key as invalid and schedules immediate
    /// garbage collection. The garbage collector removes the invali‐
    /// dated key from all keyrings and deletes the key when  its  refer‐
    /// ence count reaches zero. After this operation, the key will be
    /// ignored by all searches, even if it is not yet deleted.
    ///
    /// Keys that are marked invalid become invisible to normal key oper‐
    /// ations  immediately,  though they are still visible in `/proc/keys`
    /// (marked with an 'i' flag) until they are actually removed.
    pub fn invalidate(&self) -> Result<(), KeyError> {
        ffi::keyctl!(
            KeyCtlOperation::Invalidate,
            self.0.as_raw_id() as libc::c_ulong
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KeyRing, KeyRingIdentifier, KeyType, Permission};
    use zeroize::Zeroizing;

    #[test]
    fn test_from_raw_id() {
        let raw: i32 = 0x12345;
        let _key = Key::from_id(raw.into());
    }

    #[test]
    fn test_metadata() {
        let secret = "Test Data";

        // Obtain the default User keyring
        let ring = KeyRing::from_special_id(KeyRingIdentifier::Session, false).unwrap();

        // Create the key
        let key = ring.add_key("my-info-key", secret).unwrap();

        // Obtain and verify the info
        let info = key.metadata().unwrap();
        assert_eq!(info.get_type(), KeyType::User);
        assert_eq!(info.get_uid(), unsafe { libc::geteuid() });
        assert_eq!(info.get_gid(), unsafe { libc::getegid() });
        assert_eq!(info.get_perms().bits(), 0x3F010000);
        assert_eq!(info.get_description(), "my-info-key");

        // Cleanup
        key.invalidate().unwrap()
    }

    #[test]
    fn test_read_into_vec() {
        let secret = "Test Data";

        // Obtain the default User keyring
        let ring = KeyRing::from_special_id(KeyRingIdentifier::Session, false).unwrap();

        // Create the key
        let key = ring.add_key("vec-read-key", secret).unwrap();

        // Verify the payload
        let payload = key.read_to_vec().unwrap();
        assert_eq!(secret.as_bytes(), &payload);
        key.invalidate().unwrap();
    }

    #[test]
    fn test_user_keyring_add_key() {
        let secret = "Test Data";

        // Obtain the default User keyring
        let ring = KeyRing::from_special_id(KeyRingIdentifier::Session, false).unwrap();

        // Create the key
        let key = ring.add_key("my-super-secret-test-key", secret).unwrap();

        // A buffer that is ensured to be zeroed when
        // out of scope
        let mut buf = Zeroizing::new([0u8; 4096]);

        // Allow P/U/G full permissions
        let mut perms = KeyPermissions::new();
        perms.set_posessor_perms(Permission::ALL);
        perms.set_user_perms(Permission::ALL);
        perms.set_group_perms(Permission::ALL);

        // Set the permissions
        key.set_perms(perms).unwrap();

        // Read the secret and verify it matches
        let len = key.read(&mut buf).unwrap();
        assert_eq!(secret.as_bytes(), &buf[..len]);

        // Update it
        key.update(&"wow".as_bytes()).unwrap();

        // Verify it matches the new content
        let len = key.read(&mut buf).unwrap();
        assert_eq!("wow".as_bytes(), &buf[..len]);
        key.invalidate().unwrap()
    }
}
