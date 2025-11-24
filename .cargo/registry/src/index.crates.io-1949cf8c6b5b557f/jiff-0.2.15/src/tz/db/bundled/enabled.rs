use crate::tz::{db::special_time_zone, TimeZone, TimeZoneNameIter};

pub(crate) struct Database;

impl Database {
    pub(crate) fn new() -> Database {
        Database
    }

    pub(crate) fn reset(&self) {
        #[cfg(feature = "std")]
        self::global::clear();
    }

    pub(crate) fn get(&self, name: &str) -> Option<TimeZone> {
        #[cfg(feature = "std")]
        if let Some(tz) = self::global::get(name) {
            return Some(tz);
        }
        if let Some(tz) = special_time_zone(name) {
            return Some(tz);
        }
        let (canonical_name, tzif) = lookup(name)?;
        let tz = match TimeZone::tzif(canonical_name, tzif) {
            Ok(tz) => tz,
            Err(_err) => {
                warn!(
                    "failed to parse TZif data from bundled \
                     tzdb for time zone {canonical_name} \
                     (this is like a bug, please report it): {_err}"
                );
                return None;
            }
        };
        #[cfg(feature = "std")]
        self::global::add(canonical_name, &tz);
        Some(tz)
    }

    pub(crate) fn available<'d>(&'d self) -> TimeZoneNameIter<'d> {
        TimeZoneNameIter::from_iter(available())
    }

    pub(crate) fn is_definitively_empty(&self) -> bool {
        false
    }
}

impl core::fmt::Debug for Database {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Bundled(available)")
    }
}

fn available() -> impl Iterator<Item = &'static str> {
    #[cfg(feature = "tzdb-bundle-always")]
    {
        jiff_tzdb::available()
    }
    #[cfg(not(feature = "tzdb-bundle-always"))]
    {
        jiff_tzdb_platform::jiff_tzdb::available()
    }
}

fn lookup(name: &str) -> Option<(&'static str, &'static [u8])> {
    #[cfg(feature = "tzdb-bundle-always")]
    {
        jiff_tzdb::get(name)
    }
    #[cfg(not(feature = "tzdb-bundle-always"))]
    {
        jiff_tzdb_platform::jiff_tzdb::get(name)
    }
}

/// A simple global cache of parsed time zones.
///
/// When tzdb is bundled, all of the binary data is in the binary. But in order
/// to use it, it needs to be parsed. And parsing takes non-trivial work.
///
/// When std is enabled, we can keep a global cache of parsed time zones.
/// Currently, it is very simple and can never contract. This probably isn't
/// a big deal in practice since even if all time zones are loaded into memory,
/// its total memory usage is likely insignificant in most environments where
/// `std` is supported.
#[cfg(feature = "std")]
mod global {
    use std::{string::String, string::ToString, sync::RwLock, vec::Vec};

    use crate::{tz::TimeZone, util::utf8};

    static CACHED_ZONES: RwLock<CachedZones> =
        RwLock::new(CachedZones { zones: Vec::new() });

    /// Returns a cached time zone for the given name if it exists.
    ///
    /// If the time zone isn't in the cache, then this returns `None`.
    pub(super) fn get(name: &str) -> Option<TimeZone> {
        CACHED_ZONES.read().unwrap().get(name).cloned()
    }

    /// Associates the given time zone name with the given time zone in this
    /// cache.
    ///
    /// If the given time zone is already cached, then this is a no-op.
    ///
    /// The only way a time zone can be remove from the cache is if it's
    /// overwritten or if the cache is cleared entirely.
    pub(super) fn add(name: &str, tz: &TimeZone) {
        let mut cache = CACHED_ZONES.write().unwrap();
        if let Err(i) = cache.get_zone_index(name) {
            cache.zones.insert(
                i,
                CachedZone { name: name.to_string(), tz: tz.clone() },
            );
        }
    }

    /// Clear the entire global cache.
    pub(super) fn clear() {
        CACHED_ZONES.write().unwrap().clear();
    }

    #[derive(Debug)]
    struct CachedZones {
        zones: Vec<CachedZone>,
    }

    impl CachedZones {
        fn get(&self, query: &str) -> Option<&TimeZone> {
            self.get_zone_index(query).ok().map(|i| &self.zones[i].tz)
        }

        fn get_zone_index(&self, query: &str) -> Result<usize, usize> {
            self.zones.binary_search_by(|entry| {
                utf8::cmp_ignore_ascii_case(&entry.name, query)
            })
        }

        fn clear(&mut self) {
            self.zones.clear();
        }
    }

    #[derive(Debug)]
    struct CachedZone {
        name: String,
        tz: TimeZone,
    }
}
