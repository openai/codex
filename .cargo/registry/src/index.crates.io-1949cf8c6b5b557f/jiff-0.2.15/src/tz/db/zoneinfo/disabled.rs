use crate::tz::{TimeZone, TimeZoneNameIter};

#[derive(Clone)]
pub(crate) struct Database;

impl Database {
    pub(crate) fn from_env() -> Database {
        Database
    }

    #[cfg(feature = "std")]
    pub(crate) fn from_dir(
        dir: &std::path::Path,
    ) -> Result<Database, crate::Error> {
        Err(crate::error::err!(
            "system tzdb unavailable: \
             crate feature `tzdb-zoneinfo` is disabled, \
             opening tzdb at {dir} has therefore failed",
            dir = dir.display(),
        ))
    }

    pub(crate) fn none() -> Database {
        Database
    }

    pub(crate) fn reset(&self) {}

    pub(crate) fn get(&self, _query: &str) -> Option<TimeZone> {
        None
    }

    pub(crate) fn available<'d>(&'d self) -> TimeZoneNameIter<'d> {
        TimeZoneNameIter::empty()
    }

    pub(crate) fn is_definitively_empty(&self) -> bool {
        true
    }
}

impl core::fmt::Debug for Database {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "ZoneInfo(unavailable)")
    }
}
