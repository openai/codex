pub(crate) use self::inner::*;

#[cfg(not(feature = "tzdb-zoneinfo"))]
#[path = "disabled.rs"]
mod inner;
#[cfg(feature = "tzdb-zoneinfo")]
#[path = "enabled.rs"]
mod inner;
