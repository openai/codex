//! Definitions ported from the C keyutils library
//!
use crate::utils::CStr;
use crate::KeyError;

/// Primary kernel identifier for a key or keyring.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct KeySerialId(pub i32);

/// Pre-defined key types the kernel understands. See `man 7 keyrings`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum KeyType {
    /// Keyrings  are  special  key  types that may contain links to sequences of other
    /// keys of any type.
    KeyRing,
    /// This is a general purpose key type whose payload may be read and updated by
    /// user-space  applications. The  key is kept entirely within kernel memory.
    /// The payload for keys of this type is a blob of arbitrary data of up to 32,767 bytes.
    User,
    /// This key type is essentially the same as "user", but it does not permit the key
    /// to read. This is suitable for storing payloads that you do not want to be
    /// readable from user space.
    Logon,
    /// This key type is similar to "user", but may hold a payload of up to 1 MiB.
    /// If the key payload is large  enough, then it may be stored encrypted in
    /// tmpfs (which can be swapped out) rather than kernel memory.
    BigKey,
}

/// Special identifiers for default keyrings. See `man 7 keyrings`.
#[allow(dead_code)]
pub enum KeyRingIdentifier {
    /// Key ID for thread-specific keyring
    Thread = -1,
    /// Key ID for process-specific keyring
    Process = -2,
    /// Key ID for session-specific keyring
    Session = -3,
    /// Key ID for UID-specific keyring
    User = -4,
    /// Key ID for UID-session keyring
    UserSession = -5,
    /// Key ID for GID-specific keyring
    Group = -6,
    /// Key ID for assumed request_key auth key
    ReqKeyAuthKey = -7,
}

#[allow(dead_code)]
pub enum DefaultKeyring {
    NoChange = -1,
    Default = 0,
    Thread = 1,
    Process = 2,
    Session = 3,
    User = 4,
    UserSession = 5,
    Group = 6,
}

#[allow(dead_code)]
#[repr(u32)]
pub enum KeyCtlOperation {
    /// Ask for a keyring's ID
    GetKeyRingId = 0,
    /// Join or start named session keyring
    JoinSessionKeyRing = 1,
    /// Update a key
    Update = 2,
    /// Revoke a key
    Revoke = 3,
    /// Set ownership of a key
    Chown = 4,
    /// Set permissions of a key
    SetPerm = 5,
    /// Describe a key
    Describe = 6,
    /// Clear contents of a keyring
    Clear = 7,
    /// Link a key into a keyring
    Link = 8,
    /// Unlink a key from a keyring
    Unlink = 9,
    /// Search for a key in a keyring
    Search = 10,
    /// Read a key or keyring's contents
    Read = 11,
    /// Instantiate a partially constructed key
    Instantiate = 12,
    /// Negate a partially constructed key
    Negate = 13,
    /// Set default request-key keyring
    SetRequestKeyKeyring = 14,
    /// Set timeout on a key
    SetTimeout = 15,
    /// Assume authority to instantiate key
    AssumeAuthority = 16,
    /// Get key security label
    GetSecurityLabel = 17,
    /// Set my session keyring on my parent process
    SessionToParent = 18,
    /// Reject a partially constructed key
    Reject = 19,
    /// Instantiate a partially constructed key
    InstantiageIov = 20,
    /// Invalidate a key
    Invalidate = 21,
    /// Get a user's persistent keyring
    GetPersistent = 22,
    /// Compute Diffie-Hellman values
    DiffieHellmanCompute = 23,
    /// Query public key parameters
    PubkeyQuery = 24,
    /// Encrypt a blob using a public key
    PubkeyEncrypt = 25,
    /// Decrypt a blob using a public key
    PubkeyDecrypt = 26,
    /// Create a public key signature
    PubkeySign = 27,
    /// Verify a public key signature
    PubkeyVerify = 28,
    /// Restrict keys allowed to link to a keyring
    RestrictKeyring = 29,
    /// Move keys between keyrings
    Move = 30,
    /// Find capabilities of keyrings subsystem
    Capabilities = 31,
    /// Watch a key or ring of keys for changes
    WatchKey = 32,
}

impl KeySerialId {
    /// Construct from a raw i32
    pub fn new(raw: i32) -> Self {
        Self(raw)
    }

    /// Allow conversion into the raw i32 for FFI
    pub fn as_raw_id(&self) -> i32 {
        self.0
    }
}

/// Perform the conversion here so that invalid KeyType strings cannot be used.
/// Using Rust's type system to ensure only valid strings are provided to the syscall.
impl From<KeyType> for &'static CStr {
    fn from(t: KeyType) -> &'static CStr {
        unsafe {
            match t {
                KeyType::KeyRing => CStr::from_bytes_with_nul_unchecked(b"keyring\0"),
                KeyType::User => CStr::from_bytes_with_nul_unchecked(b"user\0"),
                KeyType::Logon => CStr::from_bytes_with_nul_unchecked(b"logon\0"),
                KeyType::BigKey => CStr::from_bytes_with_nul_unchecked(b"big_key\0"),
            }
        }
    }
}

/// Perform the conversion here so that invalid KeyType strings cannot be used.
/// Using Rust's type system to ensure only valid strings are provided to the syscall.
impl TryFrom<&str> for KeyType {
    type Error = KeyError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let val = match s {
            "keyring" => KeyType::KeyRing,
            "user" => KeyType::User,
            "logon" => KeyType::Logon,
            "big_key" => KeyType::BigKey,
            _ => return Err(KeyError::InvalidIdentifier),
        };
        Ok(val)
    }
}

/// Allow easy conversion from i32 to KeySerialId
impl From<KeySerialId> for i32 {
    fn from(id: KeySerialId) -> i32 {
        id.0
    }
}

/// Direct conversion
impl From<i32> for KeySerialId {
    fn from(n: i32) -> Self {
        Self(n)
    }
}

/// Allow easy conversion from u64 to KeySerialId
impl TryFrom<i64> for KeySerialId {
    type Error = KeyError;

    fn try_from(n: i64) -> Result<Self, Self::Error> {
        Ok(Self(n.try_into().or(Err(KeyError::InvalidIdentifier))?))
    }
}
