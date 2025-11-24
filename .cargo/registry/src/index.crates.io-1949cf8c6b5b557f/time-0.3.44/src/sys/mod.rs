//! Functions with a common interface that rely on system calls.

#[cfg(feature = "local-offset")]
mod local_offset_at;
#[cfg(feature = "local-offset")]
mod refresh_tz;

#[cfg(feature = "local-offset")]
pub(crate) use self::local_offset_at::local_offset_at;
#[cfg(feature = "local-offset")]
pub(crate) use self::refresh_tz::{refresh_tz, refresh_tz_unchecked};
