use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use std::{
    ffi::OsString,
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::{
    error::{err, Error},
    timestamp::Timestamp,
    tz::{
        concatenated::ConcatenatedTzif, db::special_time_zone, TimeZone,
        TimeZoneNameIter,
    },
    util::{self, array_str::ArrayStr, cache::Expiration, utf8},
};

const DEFAULT_TTL: Duration = Duration::new(5 * 60, 0);

/// The places to look for a concatenated `tzdata` file.
static TZDATA_LOCATIONS: &[TzdataLocation] = &[
    TzdataLocation::Env {
        name: "ANDROID_ROOT",
        default: "/system",
        suffix: "usr/share/zoneinfo/tzdata",
    },
    TzdataLocation::Env {
        name: "ANDROID_DATA",
        default: "/data/misc",
        suffix: "zoneinfo/current/tzdata",
    },
];

pub(crate) struct Database {
    path: Option<PathBuf>,
    names: Option<Names>,
    zones: RwLock<CachedZones>,
}

impl Database {
    pub(crate) fn from_env() -> Database {
        let mut attempted = vec![];
        for loc in TZDATA_LOCATIONS {
            let path = loc.to_path_buf();
            trace!(
                "opening concatenated tzdata database at {}",
                path.display()
            );
            match Database::from_path(&path) {
                Ok(db) => return db,
                Err(_err) => {
                    trace!("failed opening {}: {_err}", path.display());
                }
            }
            attempted.push(path.to_string_lossy().into_owned());
        }
        debug!(
            "could not find concatenated tzdata database at any of the \
             following paths: {}",
            attempted.join(", "),
        );
        Database::none()
    }

    pub(crate) fn from_path(path: &Path) -> Result<Database, Error> {
        let names = Some(Names::new(path)?);
        let zones = RwLock::new(CachedZones::new());
        Ok(Database { path: Some(path.to_path_buf()), names, zones })
    }

    /// Creates a "dummy" zoneinfo database in which all lookups fail.
    pub(crate) fn none() -> Database {
        let path = None;
        let names = None;
        let zones = RwLock::new(CachedZones::new());
        Database { path, names, zones }
    }

    pub(crate) fn reset(&self) {
        let mut zones = self.zones.write().unwrap();
        if let Some(ref names) = self.names {
            names.reset();
        }
        zones.reset();
    }

    pub(crate) fn get(&self, query: &str) -> Option<TimeZone> {
        if let Some(tz) = special_time_zone(query) {
            return Some(tz);
        }
        let path = self.path.as_ref()?;
        // The fast path is when the query matches a pre-existing unexpired
        // time zone.
        {
            let zones = self.zones.read().unwrap();
            if let Some(czone) = zones.get(query) {
                if !czone.is_expired() {
                    trace!(
                        "for time zone query `{query}`, \
                         found cached zone `{}` \
                         (expiration={}, last_modified={:?})",
                        czone.tz.diagnostic_name(),
                        czone.expiration,
                        czone.last_modified,
                    );
                    return Some(czone.tz.clone());
                }
            }
        }
        // At this point, one of three possible cases is true:
        //
        // 1. The given query does not match any time zone in this database.
        // 2. A time zone exists, but isn't cached.
        // 3. A zime exists and is cached, but needs to be revalidated.
        //
        // While (3) is probably the common case since our TTLs are pretty
        // short, both (2) and (3) require write access. Thus we rule out (1)
        // before acquiring a write lock on the entire database. Plus, we'll
        // need the zone info for case (2) and possibly for (3) if cache
        // revalidation fails.
        //
        // I feel kind of bad about all this because it seems to me like there
        // is too much work being done while holding on to the write lock.
        // In particular, it seems like bad juju to do any I/O of any kind
        // while holding any lock at all. I think I could design something
        // that avoids doing I/O while holding a lock, but it seems a lot more
        // complicated. (And what happens if the I/O becomes outdated by the
        // time you acquire the lock?)
        let mut zones = self.zones.write().unwrap();
        let ttl = zones.ttl;
        match zones.get_zone_index(query) {
            Ok(i) => {
                let czone = &mut zones.zones[i];
                if czone.revalidate(path, ttl) {
                    // Metadata on the file didn't change, so we assume the
                    // file hasn't either.
                    return Some(czone.tz.clone());
                }
                // Revalidation failed. Re-read the TZif data.
                let (scratch1, scratch2) = zones.scratch();
                let czone = match CachedTimeZone::new(
                    path, query, ttl, scratch1, scratch2,
                ) {
                    Ok(Some(czone)) => czone,
                    Ok(None) => return None,
                    Err(_err) => {
                        warn!(
                            "failed to re-cache time zone {query} \
                             from {path}: {_err}",
                            path = path.display(),
                        );
                        return None;
                    }
                };
                let tz = czone.tz.clone();
                zones.zones[i] = czone;
                Some(tz)
            }
            Err(i) => {
                let (scratch1, scratch2) = zones.scratch();
                let czone = match CachedTimeZone::new(
                    path, query, ttl, scratch1, scratch2,
                ) {
                    Ok(Some(czone)) => czone,
                    Ok(None) => return None,
                    Err(_err) => {
                        warn!(
                            "failed to cache time zone {query} \
                             from {path}: {_err}",
                            path = path.display(),
                        );
                        return None;
                    }
                };
                let tz = czone.tz.clone();
                zones.zones.insert(i, czone);
                Some(tz)
            }
        }
    }

    pub(crate) fn available<'d>(&'d self) -> TimeZoneNameIter<'d> {
        let Some(path) = self.path.as_ref() else {
            return TimeZoneNameIter::empty();
        };
        let Some(names) = self.names.as_ref() else {
            return TimeZoneNameIter::empty();
        };
        TimeZoneNameIter::from_iter(names.available(path).into_iter())
    }

    pub(crate) fn is_definitively_empty(&self) -> bool {
        self.names.is_none()
    }
}

impl core::fmt::Debug for Database {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Concatenated(")?;
        if let Some(ref path) = self.path {
            write!(f, "{}", path.display())?;
        } else {
            write!(f, "unavailable")?;
        }
        write!(f, ")")
    }
}

#[derive(Debug)]
struct CachedZones {
    zones: Vec<CachedTimeZone>,
    ttl: Duration,
    scratch1: Vec<u8>,
    scratch2: Vec<u8>,
}

impl CachedZones {
    const DEFAULT_TTL: Duration = DEFAULT_TTL;

    fn new() -> CachedZones {
        CachedZones {
            zones: vec![],
            ttl: CachedZones::DEFAULT_TTL,
            scratch1: vec![],
            scratch2: vec![],
        }
    }

    fn get(&self, query: &str) -> Option<&CachedTimeZone> {
        self.get_zone_index(query).ok().map(|i| &self.zones[i])
    }

    fn get_zone_index(&self, query: &str) -> Result<usize, usize> {
        self.zones.binary_search_by(|zone| {
            utf8::cmp_ignore_ascii_case(zone.name(), query)
        })
    }

    fn reset(&mut self) {
        self.zones.clear();
    }

    fn scratch(&mut self) -> (&mut Vec<u8>, &mut Vec<u8>) {
        (&mut self.scratch1, &mut self.scratch2)
    }
}

#[derive(Clone, Debug)]
struct CachedTimeZone {
    tz: TimeZone,
    expiration: Expiration,
    last_modified: Option<Timestamp>,
}

impl CachedTimeZone {
    /// Create a new cached time zone.
    ///
    /// `path` should be a concatenated `tzdata` file. `query` is the IANA time
    /// zone identifier we're looing for. The `ttl` says how long
    /// the cached time zone should minimally remain fresh for.
    ///
    /// The `scratch1` and `scratch2` given are used to help amortize
    /// allocation when deserializing TZif data from the concatenated `tzdata`
    /// file.
    ///
    /// If no such time zone exists and no other error occurred, then
    /// `Ok(None)` is returned.
    fn new(
        path: &Path,
        query: &str,
        ttl: Duration,
        scratch1: &mut Vec<u8>,
        scratch2: &mut Vec<u8>,
    ) -> Result<Option<CachedTimeZone>, Error> {
        let file = File::open(path).map_err(|e| Error::io(e).path(path))?;
        let db = ConcatenatedTzif::open(&file)?;
        let Some(tz) = db.get(query, scratch1, scratch2)? else {
            return Ok(None);
        };
        let last_modified = util::fs::last_modified_from_file(path, &file);
        let expiration = Expiration::after(ttl);
        Ok(Some(CachedTimeZone { tz, expiration, last_modified }))
    }

    /// Returns true if this time zone has gone stale and should, at minimum,
    /// be revalidated.
    fn is_expired(&self) -> bool {
        self.expiration.is_expired()
    }

    /// Returns the IANA time zone identifier of this cached time zone.
    fn name(&self) -> &str {
        // OK because `ConcatenatedTzif` guarantees all `TimeZone` values it
        // returns have an IANA name.
        self.tz.iana_name().unwrap()
    }

    /// Attempts to revalidate this cached time zone.
    ///
    /// Upon successful revalidation (that is, the cached time zone is still
    /// fresh and okay to use), this returns true. Otherwise, the cached time
    /// zone should be considered stale and must be re-created.
    ///
    /// Note that technically another layer of revalidation could be done.
    /// For example, we could keep a checksum of the TZif data, and only
    /// consider rebuilding the time zone when the checksum changes. But I
    /// think the last modified metadata will in practice be good enough, and
    /// parsing TZif data should be quite fast.
    ///
    /// `path` should be a concatenated `tzdata` file.
    fn revalidate(&mut self, path: &Path, ttl: Duration) -> bool {
        // If we started with no last modified timestamp, then I guess we
        // should always fail revalidation? I suppose a case could be made to
        // do the opposite: always pass revalidation.
        let Some(old_last_modified) = self.last_modified else {
            trace!(
                "revalidation for {name} in {path} failed because \
                 old last modified time is unavailable",
                name = self.name(),
                path = path.display(),
            );
            return false;
        };
        let Some(new_last_modified) = util::fs::last_modified_from_path(path)
        else {
            trace!(
                "revalidation for {name} in {path} failed because \
                 new last modified time is unavailable",
                name = self.name(),
                path = path.display(),
            );
            return false;
        };
        // We consider any change to invalidate cache.
        if old_last_modified != new_last_modified {
            trace!(
                "revalidation for {name} in {path} failed because \
                 last modified times do not match: old = {old} != {new} = new",
                name = self.name(),
                path = path.display(),
                old = old_last_modified,
                new = new_last_modified,
            );
            return false;
        }
        trace!(
            "revalidation for {name} in {path} succeeded because \
             last modified times match: old = {old} == {new} = new",
            name = self.name(),
            path = path.display(),
            old = old_last_modified,
            new = new_last_modified,
        );
        self.expiration = Expiration::after(ttl);
        true
    }
}

/// A collection of time zone names extracted from a concatenated tzdata file.
///
/// This type is responsible not just for providing the names, but also for
/// updating them periodically.
///
/// Every name _should_ correspond to an entry in the data block of the
/// corresponding `tzdata` file, but we generally don't take advantage of this.
/// The reason is that the file could theoretically change. Between when we
/// extract the names and when we do a TZif lookup later. This is all perfectly
/// manageable, but it should only be done if there's a benchmark demanding
/// more effort be spent here. As it stands, we do have a rudimentary caching
/// mechanism, so not all time zone lookups go through this slower path. (This
/// is also why `Names` has no lookup routine. There's just a routine to return
/// all names.)
#[derive(Debug)]
struct Names {
    inner: RwLock<NamesInner>,
}

#[derive(Debug)]
struct NamesInner {
    /// All available names from the `tzdata` file.
    names: Vec<Arc<str>>,
    /// The version string read from the `tzdata` file.
    version: ArrayStr<5>,
    /// Scratch space used to help amortize allocation when extracting names
    /// from a `tzdata` file.
    scratch: Vec<u8>,
    /// The expiration time of these cached names.
    ///
    /// Note that this is a necessary but not sufficient criterion for
    /// invalidating the cached value.
    ttl: Duration,
    /// The time at which the data in `names` becomes stale.
    expiration: Expiration,
}

impl Names {
    /// See commnents in `tz/db/zoneinfo/enabled.rs` about this. We just copied
    /// it from there.
    const DEFAULT_TTL: Duration = DEFAULT_TTL;

    /// Create a new collection of names from the concatenated `tzdata` file
    /// path given.
    ///
    /// If no names of time zones could be found in the given directory, then
    /// an error is returned.
    fn new(path: &Path) -> Result<Names, Error> {
        let path = path.to_path_buf();
        let mut scratch = vec![];
        let (names, version) = read_names_and_version(&path, &mut scratch)?;
        trace!(
            "found concatenated tzdata at {path} \
             with version {version} and {len} \
             IANA time zone identifiers",
            path = path.display(),
            len = names.len(),
        );
        let ttl = Names::DEFAULT_TTL;
        let expiration = Expiration::after(ttl);
        let inner = NamesInner { names, version, scratch, ttl, expiration };
        Ok(Names { inner: RwLock::new(inner) })
    }

    /// Returns all available time zone names after attempting a refresh of
    /// the underlying data if it's stale.
    fn available(&self, path: &Path) -> Vec<String> {
        let mut inner = self.inner.write().unwrap();
        inner.attempt_refresh(path);
        inner.available()
    }

    fn reset(&self) {
        self.inner.write().unwrap().reset();
    }
}

impl NamesInner {
    /// Returns all available time zone names.
    fn available(&self) -> Vec<String> {
        self.names.iter().map(|name| name.to_string()).collect()
    }

    /// Attempts a refresh, but only follows through if the TTL has been
    /// exceeded.
    ///
    /// The caller must ensure that the other cache invalidation criteria
    /// have been upheld. For example, this should only be called for a missed
    /// zone name lookup.
    fn attempt_refresh(&mut self, path: &Path) {
        if self.expiration.is_expired() {
            self.refresh(path);
        }
    }

    /// Forcefully refreshes the cached names with possibly new data from disk.
    /// If an error occurs when fetching the names, then no names are updated
    /// (but the `expires_at` is updated). This will also emit a warning log on
    /// failure.
    fn refresh(&mut self, path: &Path) {
        // PERF: Should we try to move this tzdb handling to run outside of a
        // lock? It probably happens pretty rarely, so it might not matter.
        let result = read_names_and_version(path, &mut self.scratch);
        self.expiration = Expiration::after(self.ttl);
        match result {
            Ok((names, version)) => {
                trace!(
                    "refreshed concatenated tzdata at {path} \
                     with version {version} and {len} \
                     IANA time zone identifiers",
                    path = path.display(),
                    len = names.len(),
                );
                self.names = names;
                self.version = version;
            }
            Err(_err) => {
                warn!(
                    "failed to refresh concatenated time zone name cache \
                     for {path}: {_err}",
                    path = path.display(),
                )
            }
        }
    }

    /// Resets the state such that the next lookup is guaranteed to force a
    /// cache refresh, and that it is impossible for any data to be stale.
    fn reset(&mut self) {
        // This will force the next lookup to fail.
        self.names.clear();
        // And this will force the next failed lookup to result in a refresh.
        self.expiration = Expiration::expired();
    }
}

/// A type representing how to find a `tzdata` file.
///
/// This currently only supports an Android-centric lookup via env vars, but if
/// we wanted to check a fixed path like we do for `ZoneInfo`, then adding a
/// `Fixed` variant here would be appropriate.
#[derive(Debug)]
enum TzdataLocation {
    Env { name: &'static str, default: &'static str, suffix: &'static str },
}

impl TzdataLocation {
    /// Converts this location to an actual path, which might involve an
    /// environment variable lookup.
    fn to_path_buf(&self) -> PathBuf {
        match *self {
            TzdataLocation::Env { name, default, suffix } => {
                let var = std::env::var_os(name)
                    .unwrap_or_else(|| OsString::from(default));
                let prefix = PathBuf::from(var);
                prefix.join(suffix)
            }
        }
    }
}

/// Reads only the IANA time zone identifiers from the given path (and the
/// version of the database).
///
/// The `scratch` given is used to help amortize allocation when deserializing
/// names from the concatenated `tzdata` file.
///
/// This returns an error if reading was successful but no names were found.
fn read_names_and_version(
    path: &Path,
    scratch: &mut Vec<u8>,
) -> Result<(Vec<Arc<str>>, ArrayStr<5>), Error> {
    let file = File::open(path).map_err(|e| Error::io(e).path(path))?;
    let db = ConcatenatedTzif::open(file)?;
    let names: Vec<Arc<str>> =
        db.available(scratch)?.into_iter().map(Arc::from).collect();
    if names.is_empty() {
        return Err(err!(
            "found no IANA time zone identifiers in \
             concatenated tzdata file at {path}",
            path = path.display(),
        ));
    }
    Ok((names, db.version()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DEBUG COMMAND
    ///
    /// Takes environment variable `JIFF_DEBUG_ZONEINFO_DIR` as input and
    /// prints a list of all time zone names in the directory (one per line).
    ///
    /// Callers may also set `RUST_LOG` to get extra debugging output.
    #[test]
    fn debug_tzdata_list() -> anyhow::Result<()> {
        let _ = crate::logging::Logger::init();

        const ENV: &str = "JIFF_DEBUG_CONCATENATED_TZDATA";
        let Some(val) = std::env::var_os(ENV) else { return Ok(()) };
        let path = PathBuf::from(val);
        let db = Database::from_path(&path)?;
        for name in db.available() {
            std::eprintln!("{name}");
        }
        Ok(())
    }
}
