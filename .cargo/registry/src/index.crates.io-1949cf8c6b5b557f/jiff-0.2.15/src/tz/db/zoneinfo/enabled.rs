use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use std::{
    ffi::OsStr,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};

use crate::{
    error::{err, Error},
    timestamp::Timestamp,
    tz::{
        db::special_time_zone, tzif::is_possibly_tzif, TimeZone,
        TimeZoneNameIter,
    },
    util::{self, cache::Expiration, parse, utf8},
};

const DEFAULT_TTL: Duration = Duration::new(5 * 60, 0);

#[cfg(unix)]
static ZONEINFO_DIRECTORIES: &[&str] =
    &["/usr/share/zoneinfo", "/usr/share/lib/zoneinfo", "/etc/zoneinfo"];

// In non-Unix environments, there is (as of 2025-05-17) no standard location
// for the zoneinfo database. And we specifically do not search the Unix-style
// directories because this can have weird and undesirable effects on Windows.
//
// Ref https://github.com/BurntSushi/jiff/issues/376
#[cfg(not(unix))]
static ZONEINFO_DIRECTORIES: &[&str] = &[];

pub(crate) struct Database {
    dir: Option<PathBuf>,
    names: Option<ZoneInfoNames>,
    zones: RwLock<CachedZones>,
}

impl Database {
    pub(crate) fn from_env() -> Database {
        if let Some(tzdir) = std::env::var_os("TZDIR") {
            let tzdir = PathBuf::from(tzdir);
            trace!("opening zoneinfo database at TZDIR={}", tzdir.display());
            match Database::from_dir(&tzdir) {
                Ok(db) => return db,
                Err(_err) => {
                    // This is a WARN because it represents a failure to
                    // satisfy a more direct request, which should be louder
                    // than failures related to auto-detection.
                    warn!("failed opening TZDIR={}: {_err}", tzdir.display());
                    // fall through to attempt default directories
                }
            }
        }
        for dir in ZONEINFO_DIRECTORIES {
            let tzdir = Path::new(dir);
            trace!("opening zoneinfo database at {}", tzdir.display());
            match Database::from_dir(&tzdir) {
                Ok(db) => return db,
                Err(_err) => {
                    trace!("failed opening {}: {_err}", tzdir.display());
                }
            }
        }
        debug!(
            "could not find zoneinfo database at any of the following \
             paths: {}",
            ZONEINFO_DIRECTORIES.join(", "),
        );
        Database::none()
    }

    pub(crate) fn from_dir(dir: &Path) -> Result<Database, Error> {
        let names = Some(ZoneInfoNames::new(dir)?);
        let zones = RwLock::new(CachedZones::new());
        Ok(Database { dir: Some(dir.to_path_buf()), names, zones })
    }

    /// Creates a "dummy" zoneinfo database in which all lookups fail.
    pub(crate) fn none() -> Database {
        let dir = None;
        let names = None;
        let zones = RwLock::new(CachedZones::new());
        Database { dir, names, zones }
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
        // If we couldn't build any time zone names, then every lookup will
        // fail. So just bail now.
        let names = self.names.as_ref()?;
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
        let info = names.get(query)?;
        let mut zones = self.zones.write().unwrap();
        let ttl = zones.ttl;
        match zones.get_zone_index(query) {
            Ok(i) => {
                let czone = &mut zones.zones[i];
                if czone.revalidate(&info, ttl) {
                    // Metadata on the file didn't change, so we assume the
                    // file hasn't either.
                    return Some(czone.tz.clone());
                }
                // Revalidation failed. Re-read the TZif data.
                let czone = match CachedTimeZone::new(&info, zones.ttl) {
                    Ok(czone) => czone,
                    Err(_err) => {
                        warn!(
                            "failed to re-cache time zone from file {}: {_err}",
                            info.inner.full.display(),
                        );
                        return None;
                    }
                };
                let tz = czone.tz.clone();
                zones.zones[i] = czone;
                Some(tz)
            }
            Err(i) => {
                let czone = match CachedTimeZone::new(&info, ttl) {
                    Ok(czone) => czone,
                    Err(_err) => {
                        warn!(
                            "failed to cache time zone from file {}: {_err}",
                            info.inner.full.display(),
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
        let Some(names) = self.names.as_ref() else {
            return TimeZoneNameIter::empty();
        };
        TimeZoneNameIter::from_iter(names.available().into_iter())
    }

    pub(crate) fn is_definitively_empty(&self) -> bool {
        self.names.is_none()
    }
}

impl core::fmt::Debug for Database {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "ZoneInfo(")?;
        if let Some(ref dir) = self.dir {
            write!(f, "{}", dir.display())?;
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
}

impl CachedZones {
    const DEFAULT_TTL: Duration = DEFAULT_TTL;

    fn new() -> CachedZones {
        CachedZones { zones: vec![], ttl: CachedZones::DEFAULT_TTL }
    }

    fn get(&self, query: &str) -> Option<&CachedTimeZone> {
        self.get_zone_index(query).ok().map(|i| &self.zones[i])
    }

    fn get_zone_index(&self, query: &str) -> Result<usize, usize> {
        // The common case is that our query matches the time zone name case
        // sensitively, so check for that first. It's a bit cheaper than doing
        // a case insensitive search.
        if let Ok(i) = self
            .zones
            .binary_search_by(|zone| zone.name.original().cmp(&query))
        {
            return Ok(i);
        }
        self.zones.binary_search_by(|zone| {
            utf8::cmp_ignore_ascii_case(zone.name.lower(), query)
        })
    }

    fn reset(&mut self) {
        self.zones.clear();
    }
}

#[derive(Clone, Debug)]
struct CachedTimeZone {
    tz: TimeZone,
    name: ZoneInfoName,
    expiration: Expiration,
    last_modified: Option<Timestamp>,
}

impl CachedTimeZone {
    /// Create a new cached time zone.
    ///
    /// The `info` says which time zone to create and where to find it. The
    /// `ttl` says how long the cached time zone should minimally remain fresh
    /// for.
    fn new(
        info: &ZoneInfoName,
        ttl: Duration,
    ) -> Result<CachedTimeZone, Error> {
        fn imp(
            info: &ZoneInfoName,
            ttl: Duration,
        ) -> Result<CachedTimeZone, Error> {
            let path = info.path();
            let mut file =
                File::open(path).map_err(|e| Error::io(e).path(path))?;
            let mut data = vec![];
            file.read_to_end(&mut data)
                .map_err(|e| Error::io(e).path(path))?;
            let tz = TimeZone::tzif(&info.inner.original, &data)
                .map_err(|e| e.path(path))?;
            let name = info.clone();
            let last_modified = util::fs::last_modified_from_file(path, &file);
            let expiration = Expiration::after(ttl);
            Ok(CachedTimeZone { tz, name, expiration, last_modified })
        }

        let result = imp(info, ttl);
        info.set_validity(result.is_ok());
        result
    }

    /// Returns true if this time zone has gone stale and should, at minimum,
    /// be revalidated.
    fn is_expired(&self) -> bool {
        self.expiration.is_expired()
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
    /// parsing a TZif file should be quite fast.
    fn revalidate(&mut self, info: &ZoneInfoName, ttl: Duration) -> bool {
        // If we started with no last modified timestamp, then I guess we
        // should always fail revalidation? I suppose a case could be made to
        // do the opposite: always pass revalidation.
        let Some(old_last_modified) = self.last_modified else {
            trace!(
                "revalidation for {} failed because old last modified time \
                 is unavailable",
                info.inner.full.display(),
            );
            return false;
        };
        let Some(new_last_modified) =
            util::fs::last_modified_from_path(info.path())
        else {
            trace!(
                "revalidation for {} failed because new last modified time \
                 is unavailable",
                info.inner.full.display(),
            );
            return false;
        };
        // We consider any change to invalidate cache.
        if old_last_modified != new_last_modified {
            trace!(
                "revalidation for {} failed because last modified times \
                 do not match: old = {} != {} = new",
                info.inner.full.display(),
                old_last_modified,
                new_last_modified,
            );
            return false;
        }
        trace!(
            "revalidation for {} succeeded because last modified times \
             match: old = {} == {} = new",
            info.inner.full.display(),
            old_last_modified,
            new_last_modified,
        );
        self.expiration = Expiration::after(ttl);
        true
    }
}

/// A collection of time zone names extracted from a zoneinfo directory.
///
/// Each time zone name maps to a full path on the file system corresponding
/// to the TZif formatted data file for that time zone.
///
/// This type is responsible not just for providing the names, but also for
/// updating them periodically.
#[derive(Debug)]
struct ZoneInfoNames {
    inner: RwLock<ZoneInfoNamesInner>,
}

#[derive(Debug)]
struct ZoneInfoNamesInner {
    /// The directory from which we collected time zone names.
    dir: PathBuf,
    /// All available names from the `zoneinfo` directory.
    ///
    /// Each name corresponds to the suffix of a file path
    /// starting with `dir`. For example, `America/New_York` in
    /// `/usr/share/zoneinfo/America/New_York`. Each name also has a normalized
    /// lowercase version of the name for easy case insensitive lookup.
    names: Vec<ZoneInfoName>,
    /// The expiration time of this cached value.
    ///
    /// Note that this is a necessary but not sufficient criterion for
    /// invalidating the cached value.
    ttl: Duration,
    /// The time at which the data in `names` becomes stale.
    expiration: Expiration,
}

impl ZoneInfoNames {
    /// The default amount of time to wait before checking for added/removed
    /// time zones.
    ///
    /// Note that this TTL is a necessary but not sufficient criterion to
    /// provoke cache invalidation. Namely, since we don't expect the set of
    /// possible time zone names to change often, we only invalidate the cache
    /// under these circumstances:
    ///
    /// 1. The TTL or more has passed since the last time the names were
    /// attempted to be refreshed (even if it wasn't successful).
    /// 2. A name lookup is attempted and it isn't found. This is required
    /// because otherwise there isn't much point in refreshing the names.
    ///
    /// This logic does not deal as well with removals from the underlying time
    /// zone database. That in turn is covered by the TTL on constructing the
    /// `TimeZone` values themselves.
    ///
    /// We could just use the second criterion on its own, but we require the
    /// TTL to expire out of "good sense." Namely, if there is something borked
    /// in the environment, the TTL will prevent doing a full scan of the
    /// zoneinfo directory for every missed time zone lookup.
    const DEFAULT_TTL: Duration = DEFAULT_TTL;

    /// Create a new collection of names from the zoneinfo database directory
    /// given.
    ///
    /// If no names of time zones with corresponding TZif data files could be
    /// found in the given directory, then an error is returned.
    fn new(dir: &Path) -> Result<ZoneInfoNames, Error> {
        let names = walk(dir)?;
        let dir = dir.to_path_buf();
        let ttl = ZoneInfoNames::DEFAULT_TTL;
        let expiration = Expiration::after(ttl);
        let inner = ZoneInfoNamesInner { dir, names, ttl, expiration };
        Ok(ZoneInfoNames { inner: RwLock::new(inner) })
    }

    /// Attempts to find the name entry for the given query using a case
    /// insensitive search.
    ///
    /// If no match is found and the data is stale, then the time zone names
    /// are refreshed from the file system before doing another check.
    fn get(&self, query: &str) -> Option<ZoneInfoName> {
        {
            let inner = self.inner.read().unwrap();
            if let Some(zone_info_name) = inner.get(query) {
                return Some(zone_info_name);
            }
            drop(inner); // unlock
        }
        let mut inner = self.inner.write().unwrap();
        inner.attempt_refresh();
        inner.get(query)
    }

    /// Returns all available time zone names after attempting a refresh of
    /// the underlying data if it's stale.
    fn available(&self) -> Vec<String> {
        let mut inner = self.inner.write().unwrap();
        inner.attempt_refresh();
        inner.available()
    }

    fn reset(&self) {
        self.inner.write().unwrap().reset();
    }
}

impl ZoneInfoNamesInner {
    /// Attempts to find the name entry for the given query using a case
    /// insensitive search.
    ///
    /// `None` is returned if one isn't found.
    fn get(&self, query: &str) -> Option<ZoneInfoName> {
        self.names
            .binary_search_by(|n| {
                utf8::cmp_ignore_ascii_case(&n.inner.lower, query)
            })
            .ok()
            .map(|i| self.names[i].clone())
    }

    /// Returns all available time zone names.
    fn available(&self) -> Vec<String> {
        self.names
            .iter()
            .filter(|n| n.is_valid())
            .map(|n| n.inner.original.clone())
            .collect()
    }

    /// Attempts a refresh, but only follows through if the TTL has been
    /// exceeded.
    ///
    /// The caller must ensure that the other cache invalidation criteria
    /// have been upheld. For example, this should only be called for a missed
    /// zone name lookup.
    fn attempt_refresh(&mut self) {
        if self.expiration.is_expired() {
            self.refresh();
        }
    }

    /// Forcefully refreshes the cached names with possibly new data from disk.
    /// If an error occurs when fetching the names, then no names are updated
    /// (but the `expires_at` is updated). This will also emit a warning log on
    /// failure.
    fn refresh(&mut self) {
        // PERF: Should we try to move this `walk` call to run outside of a
        // lock? It probably happens pretty rarely, so it might not matter.
        let result = walk(&self.dir);
        self.expiration = Expiration::after(self.ttl);
        match result {
            Ok(names) => {
                self.names = names;
            }
            Err(_err) => {
                warn!(
                    "failed to refresh zoneinfo time zone name cache \
                     for {}: {_err}",
                    self.dir.display(),
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

/// A single TZif entry in a zoneinfo database directory.
#[derive(Clone, Debug)]
struct ZoneInfoName {
    inner: Arc<ZoneInfoNameInner>,
}

#[derive(Debug)]
struct ZoneInfoNameInner {
    /// A file path resolvable to the corresponding file relative to the
    /// working directory of this program.
    ///
    /// Should we canonicalize this to a absolute path? I guess in practice it
    /// is an absolute path in most cases.
    full: PathBuf,
    /// The original name of this time zone taken from the file path with
    /// no additional changes.
    original: String,
    /// The lowercase version of `original`. This is how we determine name
    /// equality.
    lower: String,
    /// The known validity state of this time zone name. `0` means unknown (and
    /// thus we need to check), `1` means "presumably valid" and `2` means
    /// "known invalid." The "presumably valid" means that the file has a
    /// 4-byte TZif header and the odds of a false positive a low enough that
    /// we can and should behave as if that file is actually TZif and thus a
    /// valid IANA time zone identifier.
    validity: AtomicUsize,
}

impl ZoneInfoName {
    /// Create a new time zone info name.
    ///
    /// `base` should corresponding to the zoneinfo directory from which the
    /// suffix `time_zone_name` path was returned.
    fn new(base: &Path, time_zone_name: &Path) -> Result<ZoneInfoName, Error> {
        let full = base.join(time_zone_name);
        let original = parse::os_str_utf8(time_zone_name.as_os_str())
            .map_err(|err| err.path(base))?;
        let lower = original.to_ascii_lowercase();
        let inner = ZoneInfoNameInner {
            full,
            original: original.to_string(),
            lower,
            validity: AtomicUsize::new(ZONE_INFO_NAME_UNKNOWN),
        };
        Ok(ZoneInfoName { inner: Arc::new(inner) })
    }

    /// Returns the path to the corresponding (presumed) TZif file.
    fn path(&self) -> &Path {
        &self.inner.full
    }

    /// Returns the original name of this time zone.
    fn original(&self) -> &str {
        &self.inner.original
    }

    /// Returns the lowercase name of this time zone.
    fn lower(&self) -> &str {
        &self.inner.lower
    }

    /// Returns true if it is presumed that this points to a valid TZif file.
    ///
    /// The result of this function may use a cached value.
    fn is_valid(&self) -> bool {
        let validity = self.inner.validity.load(Ordering::Relaxed);
        if validity == ZONE_INFO_NAME_VALID {
            return true;
        } else if validity == ZONE_INFO_NAME_INVALID {
            return false;
        }
        if self.is_valid_impl() {
            self.inner.validity.store(ZONE_INFO_NAME_VALID, Ordering::Relaxed);
            true
        } else {
            self.inner
                .validity
                .store(ZONE_INFO_NAME_INVALID, Ordering::Relaxed);
            false
        }
    }

    /// Marks this zone info name as known to be valid or invalid.
    ///
    /// e.g., After doing a successful time zone lookup.
    fn set_validity(&self, is_valid: bool) {
        let validity = if is_valid {
            ZONE_INFO_NAME_VALID
        } else {
            ZONE_INFO_NAME_INVALID
        };
        self.inner.validity.store(validity, Ordering::Relaxed);
    }

    /// Performs the actual validity check.
    ///
    /// This is generally only needed for APIs like
    /// `TimeZoneDatabase::available()`, where we don't want to load every time
    /// zone into memory but we also don't want to return IANA time zone ids
    /// like `zone.tab`. (Because a `/usr/share/zoneinfo` directory might have
    /// random junk in it.)
    fn is_valid_impl(&self) -> bool {
        let path = self.path();
        let mut f = match File::open(path) {
            Ok(f) => f,
            Err(_err) => {
                trace!("failed to open {}: {_err}", path.display());
                return false;
            }
        };
        let mut buf = [0; 4];
        if let Err(_err) = f.read_exact(&mut buf) {
            trace!(
                "failed to read first 4 bytes of {}: {_err}",
                path.display()
            );
            return false;
        }
        if !is_possibly_tzif(&buf) {
            // This is a trace because it's perfectly normal for a
            // non-TZif file to be in a zoneinfo directory. But it could
            // still be potentially useful debugging info.
            trace!(
                "found file {} that isn't TZif since its first \
                 four bytes are {:?}",
                path.display(),
                crate::util::escape::Bytes(&buf),
            );
            return false;
        }
        true
    }
}

impl Eq for ZoneInfoName {}

impl PartialEq for ZoneInfoName {
    fn eq(&self, rhs: &ZoneInfoName) -> bool {
        self.inner.lower == rhs.inner.lower
    }
}

impl Ord for ZoneInfoName {
    fn cmp(&self, rhs: &ZoneInfoName) -> core::cmp::Ordering {
        self.inner.lower.cmp(&rhs.inner.lower)
    }
}

impl PartialOrd for ZoneInfoName {
    fn partial_cmp(&self, rhs: &ZoneInfoName) -> Option<core::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

impl core::hash::Hash for ZoneInfoName {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.inner.lower.hash(state);
    }
}

static ZONE_INFO_NAME_UNKNOWN: usize = 0;
static ZONE_INFO_NAME_VALID: usize = 1;
static ZONE_INFO_NAME_INVALID: usize = 2;

/// Recursively walks the given directory and returns the names of all time
/// zones found.
///
/// This is guaranteed to return either one or more time zone names OR an
/// error. That is, `Ok(vec![])` is an impossible result.
///
/// This will attempt to collect as many names as possible, even if some I/O
/// operations fail.
///
/// The names returned are sorted in lexicographic order according to the
/// lowercase form of each name.
///
/// # Performance
///
/// Note that this routine is written in a way that, at least on Unix, we
/// should not be doing a syscall for every file. We need to do one for every
/// directory, but that should be comparatively rare. It's done this way to
/// avoid long initialization times when `/usr/share/zoneinfo` is on a slow
/// file system.
///
/// See: https://github.com/BurntSushi/jiff/issues/366
fn walk(start: &Path) -> Result<Vec<ZoneInfoName>, Error> {
    struct StackEntry {
        dir: PathBuf,
        depth: usize,
    }

    let mut first_err: Option<Error> = None;
    let mut seterr = |path: &Path, err: Error| {
        if first_err.is_none() {
            first_err = Some(err.path(path));
        }
    };

    let mut names = vec![];
    let mut stack = vec![StackEntry { dir: start.to_path_buf(), depth: 0 }];
    while let Some(StackEntry { dir, depth }) = stack.pop() {
        let readdir = match dir.read_dir() {
            Ok(readdir) => readdir,
            Err(err) => {
                info!(
                    "error when reading {} as a directory: {err}",
                    dir.display()
                );
                seterr(&dir, Error::io(err));
                continue;
            }
        };
        for result in readdir {
            let dent = match result {
                Ok(dent) => dent,
                Err(err) => {
                    info!(
                        "error when reading directory entry from {}: {err}",
                        dir.display()
                    );
                    seterr(&dir, Error::io(err));
                    continue;
                }
            };
            let file_type = match dent.file_type() {
                Ok(file_type) => file_type,
                Err(err) => {
                    let path = dent.path();
                    info!(
                        "error when reading file type from {}: {err}",
                        path.display()
                    );
                    seterr(&path, Error::io(err));
                    continue;
                }
            };
            let path = dent.path();
            if file_type.is_dir() {
                // We ignore the `posix` and `right` directories because Jiff
                // doesn't care about them. They tend to bloat the output of
                // `TimeZoneDatabase::available()` for no appreciable reason.
                // If callers want to use them, they can do, e.g.,
                // `TZDIR=/usr/share/zoneinfo/posix`. Moreover, they slow down
                // initialization time in environments with very slow file
                // systems.
                if depth == 0
                    && (dent.file_name() == OsStr::new("posix")
                        || dent.file_name() == OsStr::new("right"))
                {
                    continue;
                }
                stack.push(StackEntry {
                    dir: path,
                    depth: depth.saturating_add(1),
                });
                continue;
            }
            trace!(
                "zoneinfo database initialization visiting {path}",
                path = path.display(),
            );
            // We assume symlinks are files, although this may not be
            // appropriate. If we need to also handle the case when they're
            // directories, then we'll need to add symlink loop detection.

            let time_zone_name = match path.strip_prefix(start) {
                Ok(time_zone_name) => time_zone_name,
                Err(err) => {
                    trace!(
                        "failed to extract time zone name from {} \
                         using {} as a base: {err}",
                        path.display(),
                        start.display(),
                    );
                    seterr(&path, Error::adhoc(err));
                    continue;
                }
            };
            let zone_info_name =
                match ZoneInfoName::new(&start, time_zone_name) {
                    Ok(zone_info_name) => zone_info_name,
                    Err(err) => {
                        seterr(&path, err);
                        continue;
                    }
                };
            names.push(zone_info_name);
        }
    }
    if names.is_empty() {
        let err = first_err
            .take()
            .unwrap_or_else(|| err!("{}: no TZif files", start.display()));
        Err(err)
    } else {
        // If we found at least one valid name, then we declare success and
        // drop any error we might have found. They do all get logged above
        // though.
        names.sort();
        Ok(names)
    }
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
    fn debug_zoneinfo_walk() -> anyhow::Result<()> {
        let _ = crate::logging::Logger::init();

        const ENV: &str = "JIFF_DEBUG_ZONEINFO_DIR";
        let Some(val) = std::env::var_os(ENV) else { return Ok(()) };
        let dir = PathBuf::from(val);
        let names = walk(&dir)?;
        for n in names {
            std::eprintln!("{}", n.inner.original);
        }
        Ok(())
    }
}
