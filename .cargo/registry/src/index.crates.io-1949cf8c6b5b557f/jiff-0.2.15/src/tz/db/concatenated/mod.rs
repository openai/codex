pub(crate) use self::inner::*;

#[cfg(not(feature = "tzdb-concatenated"))]
#[path = "disabled.rs"]
mod inner;
#[cfg(feature = "tzdb-concatenated")]
#[path = "enabled.rs"]
mod inner;
