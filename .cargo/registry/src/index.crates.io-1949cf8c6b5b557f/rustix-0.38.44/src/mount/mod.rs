//! Linux `mount` API.

// The `mount` module includes the `mount` function and related
// functions which were originally defined in `rustix::fs` but are
// now replaced by deprecated aliases. After the next semver bump,
// we can remove the aliases and all the `#[cfg(feature = "mount")]`
// here and in src/backend/*/mount.
//
// The `fsopen` module includes `fsopen` and related functions.

#[cfg(feature = "mount")]
mod fsopen;
mod mount_unmount;
mod types;

#[cfg(feature = "mount")]
pub use fsopen::*;
pub use mount_unmount::*;
pub use types::*;
