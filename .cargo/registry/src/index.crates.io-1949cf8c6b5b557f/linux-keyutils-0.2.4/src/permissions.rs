//! Create a more rust-like permissions construct, ported from the unix
//! permissions defined in keyutils.h
use bitflags::bitflags;

/// Construct key permissions for use with [Key::set_perms](crate::Key::set_perms)
/// or returned by [Metadata::get_perms](crate::Metadata::get_perms).
///
/// Usage:
///
/// ```
/// use linux_keyutils::{Permission, KeyPermissions};
///
/// let mut perms = KeyPermissions::new();
/// perms.set_user_perms(Permission::ALL);
/// perms.set_group_perms(Permission::VIEW);
/// ```
#[derive(Debug, Copy, Clone)]
pub struct KeyPermissions(u32);

/// Construct key permissions with the builder pattern.
///
/// Usage:
///
/// ```
/// use linux_keyutils::{Permission, KeyPermissionsBuilder};
///
/// let perms = KeyPermissionsBuilder::builder()
///             .user(Permission::ALL)
///             .group(Permission::VIEW)
///             .build();
/// ```
#[derive(Debug, Copy, Clone)]
pub struct KeyPermissionsBuilder(KeyPermissions);

bitflags! {
    /// Pre-defined bit-flags to construct permissions easily.
    #[repr(transparent)]
    pub struct Permission: u8 {
        /// Allows viewing a key's attributes
        const VIEW = 0x1;
        /// Allows reading a key's payload / viewing a keyring
        const READ = 0x2;
        /// Allows writing/updating a key's payload / adding a link to keyring
        const WRITE = 0x4;
        /// Allows finding a key in search / searching a keyring
        const SEARCH = 0x8;
        /// Allows creating a link to a key/keyring
        const LINK = 0x10;
        /// Allows setting attributes for a key
        const SETATTR = 0x20;
        /// Allows all actions
        const ALL = 0x3f;
    }
}

impl Default for KeyPermissions {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyPermissions {
    /// Create a new KeyPermissions object, defaults to empty permissions
    pub fn new() -> Self {
        Self(0)
    }

    /// Construct the permissions manually
    pub fn from_u32(raw: u32) -> Self {
        Self(raw)
    }

    /// Obtain the u32 bits for this set
    pub fn bits(&self) -> u32 {
        self.0
    }

    /// Set the permissions available to the key's possessor
    pub fn set_posessor_perms(&mut self, perm: Permission) {
        self.0 &= !(0xFF << 24);
        self.0 += (perm.bits() as u32) << 24;
    }

    /// Set the permissions available to the key's owning user (UID)
    pub fn set_user_perms(&mut self, perm: Permission) {
        self.0 &= !(0xFF << 16);
        self.0 += (perm.bits() as u32) << 16;
    }

    /// Set the permissions available to the key's owning group (GID)
    pub fn set_group_perms(&mut self, perm: Permission) {
        self.0 &= !(0xFF << 8);
        self.0 += (perm.bits() as u32) << 8;
    }

    /// Set the permissions available to any 3rd party
    pub fn set_world_perms(&mut self, perm: Permission) {
        self.0 &= !0xFF;
        self.0 += perm.bits() as u32;
    }
}

impl KeyPermissionsBuilder {
    /// Start a KeyPermissionsBuilder
    pub fn builder() -> Self {
        Self(KeyPermissions::default())
    }

    /// Set the permissions available to the key's possessor
    pub fn posessor(mut self, perm: Permission) -> Self {
        self.0.set_posessor_perms(perm);
        self
    }

    /// Set the permissions available to the key's owning user (UID)
    pub fn user(mut self, perm: Permission) -> Self {
        self.0.set_user_perms(perm);
        self
    }

    /// Set the permissions available to the key's owning group (GID)
    pub fn group(mut self, perm: Permission) -> Self {
        self.0.set_group_perms(perm);
        self
    }

    /// Set the permissions available to any 3rd party
    pub fn world(mut self, perm: Permission) -> Self {
        self.0.set_world_perms(perm);
        self
    }

    /// Finish the build and obtain the created KeyPermissions
    pub fn build(self) -> KeyPermissions {
        self.0
    }
}

#[test]
fn test_posessor_perms() {
    let mut perm = KeyPermissions::default();

    // Initial
    perm.set_posessor_perms(Permission::ALL);
    assert_eq!(perm.0, 0x3f000000);

    // Update
    perm.set_posessor_perms(Permission::SEARCH);
    assert_eq!(perm.0, 0x08000000);

    // Combination
    perm.set_posessor_perms(Permission::SEARCH | Permission::VIEW);
    assert_eq!(perm.0, 0x09000000);

    // Combination two
    perm.set_posessor_perms(
        Permission::SETATTR | Permission::VIEW | Permission::READ | Permission::WRITE,
    );
    assert_eq!(perm.0, 0x27000000);
}

#[test]
fn test_user_perms() {
    let mut perm = KeyPermissions::default();

    // Initial
    perm.set_user_perms(Permission::ALL);
    assert_eq!(perm.0, 0x003f0000);

    // Update
    perm.set_user_perms(Permission::SEARCH);
    assert_eq!(perm.0, 0x00080000);

    // Combination
    perm.set_user_perms(Permission::SEARCH | Permission::VIEW);
    assert_eq!(perm.0, 0x00090000);

    // Combination2
    perm.set_user_perms(
        Permission::SETATTR | Permission::VIEW | Permission::READ | Permission::WRITE,
    );
    assert_eq!(perm.0, 0x00270000);
}

#[test]
fn test_group_perms() {
    let mut perm = KeyPermissions::default();

    // Initial
    perm.set_group_perms(Permission::ALL);
    assert_eq!(perm.0, 0x00003f00);

    // Update
    perm.set_group_perms(Permission::SEARCH);
    assert_eq!(perm.0, 0x00000800);

    // Combination
    perm.set_group_perms(Permission::SEARCH | Permission::VIEW);
    assert_eq!(perm.0, 0x00000900);

    // Combination2
    perm.set_group_perms(
        Permission::SETATTR | Permission::VIEW | Permission::READ | Permission::WRITE,
    );
    assert_eq!(perm.0, 0x00002700);
}

#[test]
fn test_world_perms() {
    let mut perm = KeyPermissions::default();

    // Initial
    perm.set_world_perms(Permission::ALL);
    assert_eq!(perm.0, 0x0000003f);

    // Update
    perm.set_world_perms(Permission::SEARCH);
    assert_eq!(perm.0, 0x00000008);

    // Combination
    perm.set_world_perms(Permission::SEARCH | Permission::VIEW);
    assert_eq!(perm.0, 0x00000009);

    // Combination2
    perm.set_world_perms(
        Permission::SETATTR | Permission::VIEW | Permission::READ | Permission::WRITE,
    );
    assert_eq!(perm.0, 0x00000027);
}

#[test]
fn test_combined_perms() {
    let mut perm = KeyPermissions::default();

    // Posessor
    perm.set_posessor_perms(Permission::ALL);
    assert_eq!(perm.0, 0x3f000000);

    // User
    perm.set_user_perms(Permission::VIEW | Permission::READ | Permission::WRITE);
    assert_eq!(perm.0, 0x3f070000);

    // Group
    perm.set_group_perms(Permission::SEARCH | Permission::VIEW);
    assert_eq!(perm.0, 0x3f070900);

    // World
    perm.set_world_perms(
        Permission::SETATTR | Permission::VIEW | Permission::READ | Permission::WRITE,
    );
    assert_eq!(perm.0, 0x3f070927);
}
