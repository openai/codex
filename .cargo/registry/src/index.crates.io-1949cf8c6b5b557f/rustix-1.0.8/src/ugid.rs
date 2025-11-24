//! User and Group ID types.

use crate::backend::c;
use crate::ffi;

/// A group identifier as a raw integer.
pub type RawGid = ffi::c_uint;
/// A user identifier as a raw integer.
pub type RawUid = ffi::c_uint;

/// `uid_t`—A Unix user ID.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct Uid(RawUid);

/// `gid_t`—A Unix group ID.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct Gid(RawGid);

impl Uid {
    /// A `Uid` corresponding to the root user (uid 0).
    pub const ROOT: Self = Self(0);

    /// Converts a `RawUid` into a `Uid`.
    ///
    /// `raw` must be the value of a valid Unix user ID, and not `-1`.
    #[inline]
    pub fn from_raw(raw: RawUid) -> Self {
        debug_assert_ne!(raw, !0);
        Self(raw)
    }

    /// Converts a `RawUid` into a `Uid`.
    ///
    /// `raw` must be the value of a valid Unix user ID, and not `-1`.
    #[inline]
    pub const fn from_raw_unchecked(raw: RawUid) -> Self {
        Self(raw)
    }

    /// Converts a `Uid` into a `RawUid`.
    #[inline]
    pub const fn as_raw(self) -> RawUid {
        self.0
    }

    /// Test whether this uid represents the root user ([`Uid::ROOT`]).
    #[inline]
    pub const fn is_root(self) -> bool {
        self.0 == Self::ROOT.0
    }
}

impl Gid {
    /// A `Gid` corresponding to the root group (gid 0).
    pub const ROOT: Self = Self(0);

    /// Converts a `RawGid` into a `Gid`.
    ///
    /// `raw` must be the value of a valid Unix group ID, and not `-1`.
    #[inline]
    pub fn from_raw(raw: RawGid) -> Self {
        debug_assert_ne!(raw, !0);
        Self(raw)
    }

    /// Converts a `RawGid` into a `Gid`.
    ///
    /// `raw` must be the value of a valid Unix group ID, and not `-1`.
    #[inline]
    pub const fn from_raw_unchecked(raw: RawGid) -> Self {
        Self(raw)
    }

    /// Converts a `Gid` into a `RawGid`.
    #[inline]
    pub const fn as_raw(self) -> RawGid {
        self.0
    }

    /// Test whether this gid represents the root group ([`Gid::ROOT`]).
    #[inline]
    pub const fn is_root(self) -> bool {
        self.0 == Self::ROOT.0
    }
}

// Return the raw value of the IDs. In case of `None` it returns `!0` since it
// has the same bit pattern as `-1` indicating no change to the owner/group ID.
pub(crate) fn translate_fchown_args(
    owner: Option<Uid>,
    group: Option<Gid>,
) -> (c::uid_t, c::gid_t) {
    let ow = match owner {
        Some(o) => o.as_raw(),
        None => !0,
    };

    let gr = match group {
        Some(g) => g.as_raw(),
        None => !0,
    };

    (ow as c::uid_t, gr as c::gid_t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sizes() {
        assert_eq_size!(RawUid, u32);
        assert_eq_size!(RawGid, u32);
        assert_eq_size!(RawUid, libc::uid_t);
        assert_eq_size!(RawGid, libc::gid_t);
    }
}
