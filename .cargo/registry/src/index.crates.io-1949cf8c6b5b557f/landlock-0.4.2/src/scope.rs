// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{uapi, Access, ABI};
use enumflags2::{bitflags, BitFlags};

/// Scope right.
///
/// Each variant of `Scope` is a
/// [scope flag](https://www.kernel.org/doc/html/latest/userspace-api/landlock.html#scope-flags).
/// A set of scopes can be created with [`BitFlags<Scope>`](BitFlags).
///
/// # Example
///
/// ```
/// use landlock::{ABI, Access, Scope, BitFlags, make_bitflags};
///
/// let signal = Scope::Signal;
///
/// let signal_set: BitFlags<Scope> = signal.into();
///
/// let signal_uds = make_bitflags!(Scope::{Signal | AbstractUnixSocket});
///
/// let scope_v6 = Scope::from_all(ABI::V6);
///
/// assert_eq!(signal_uds, scope_v6);
/// ```
///
/// # Warning
///
/// To avoid unknown restrictions **don't use `BitFlags::<Scope>::all()` nor `BitFlags::ALL`**,
/// but use a version you tested and vetted instead,
/// for instance [`Scope::from_all(ABI::V6)`](Access::from_all).
/// Direct use of **the [`BitFlags`] API is deprecated**.
/// See [`ABI`] for the rationale and help to test it.
#[bitflags]
#[repr(u64)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Scope {
    /// Restrict from connecting to abstract UNIX sockets created outside the sandbox.
    AbstractUnixSocket = uapi::LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET as u64,
    /// Restrict from sending signals to processes outside the sandbox.
    Signal = uapi::LANDLOCK_SCOPE_SIGNAL as u64,
}

/// # Warning
///
/// If `ABI <= ABI::V5`, `Scope::from_all()` returns an empty `BitFlags<AccessScope>`, which
/// makes `Ruleset::handle_access(AccessScope::from_all(ABI::V5))` return an error.
impl Access for Scope {
    fn from_all(abi: ABI) -> BitFlags<Self> {
        match abi {
            ABI::Unsupported | ABI::V1 | ABI::V2 | ABI::V3 | ABI::V4 | ABI::V5 => BitFlags::EMPTY,
            ABI::V6 => Scope::AbstractUnixSocket | Scope::Signal,
        }
    }
}
