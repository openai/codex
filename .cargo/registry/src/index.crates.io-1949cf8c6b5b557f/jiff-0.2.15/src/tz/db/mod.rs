use crate::{
    error::{err, Error},
    tz::TimeZone,
    util::{sync::Arc, utf8},
};

mod bundled;
mod concatenated;
mod zoneinfo;

/// Returns a copy of the global [`TimeZoneDatabase`].
///
/// This is the same database used for convenience routines like
/// [`Timestamp::in_tz`](crate::Timestamp::in_tz) and parsing routines
/// for [`Zoned`](crate::Zoned) that need to do IANA time zone identifier
/// lookups. Basically, whenever an implicit time zone database is needed,
/// it is *this* copy of the time zone database that is used.
///
/// In feature configurations where a time zone database cannot interact with
/// the file system (like when `std` is not enabled), this returns a database
/// where every lookup will fail.
///
/// # Example
///
/// ```
/// use jiff::tz;
///
/// assert!(tz::db().get("Antarctica/Troll").is_ok());
/// assert!(tz::db().get("does-not-exist").is_err());
/// ```
pub fn db() -> &'static TimeZoneDatabase {
    #[cfg(any(not(feature = "std"), miri))]
    {
        static NONE: TimeZoneDatabase = TimeZoneDatabase::none();
        &NONE
    }
    #[cfg(all(feature = "std", not(miri)))]
    {
        use std::sync::OnceLock;

        static DB: OnceLock<TimeZoneDatabase> = OnceLock::new();
        DB.get_or_init(|| {
            let db = TimeZoneDatabase::from_env();
            debug!("initialized global time zone database: {db:?}");
            db
        })
    }
}

/// A handle to a [IANA Time Zone Database].
///
/// A `TimeZoneDatabase` provides a way to lookup [`TimeZone`]s by their
/// human readable identifiers, such as `America/Los_Angeles` and
/// `Europe/Warsaw`.
///
/// It is rare to need to create or use this type directly. Routines
/// like zoned datetime parsing and time zone conversion provide
/// convenience routines for using an implicit global time zone database
/// by default. This global time zone database is available via
/// [`jiff::tz::db`](crate::tz::db()`). But lower level parsing routines
/// such as
/// [`fmt::temporal::DateTimeParser::parse_zoned_with`](crate::fmt::temporal::DateTimeParser::parse_zoned_with)
/// and
/// [`civil::DateTime::to_zoned`](crate::civil::DateTime::to_zoned) provide a
/// means to use a custom copy of a `TimeZoneDatabase`.
///
/// # Platform behavior
///
/// This behavior is subject to change.
///
/// On Unix systems, and when the `tzdb-zoneinfo` crate feature is enabled
/// (which it is by default), Jiff will read the `/usr/share/zoneinfo`
/// directory for time zone data.
///
/// On Windows systems and when the `tzdb-bundle-platform` crate feature is
/// enabled (which it is by default), _or_ when the `tzdb-bundle-always` crate
/// feature is enabled, then the `jiff-tzdb` crate will be used to embed the
/// entire Time Zone Database into the compiled artifact.
///
/// On Android systems, and when the `tzdb-concatenated` crate feature is
/// enabled (which it is by default), Jiff will attempt to read a concatenated
/// zoneinfo database using the `ANDROID_DATA` or `ANDROID_ROOT` environment
/// variables.
///
/// In general, using `/usr/share/zoneinfo` (or an equivalent) is heavily
/// preferred in lieu of embedding the database into your compiled artifact.
/// The reason is because your system copy of the Time Zone Database may be
/// updated, perhaps a few times a year, and it is better to get seamless
/// updates through your system rather than needing to wait on a Rust crate
/// to update and then rebuild your software. The bundling approach should
/// only be used when there is no plausible alternative. For example, Windows
/// has no canonical location for a copy of the Time Zone Database. Indeed,
/// this is why the Cargo configuration of Jiff specifically does not enabled
/// bundling by default on Unix systems, but does enable it by default on
/// Windows systems. Of course, if you really do need a copy of the database
/// bundled, then you can enable the `tzdb-bundle-always` crate feature.
///
/// # Cloning
///
/// A `TimeZoneDatabase` can be cheaply cloned. It will share a thread safe
/// cache with other copies of the same `TimeZoneDatabase`.
///
/// # Caching
///
/// Because looking up a time zone on disk, reading the file into memory
/// and parsing the time zone transitions out of that file requires
/// a fair amount of work, a `TimeZoneDatabase` does a fair bit of
/// caching. This means that the vast majority of calls to, for example,
/// [`Timestamp::in_tz`](crate::Timestamp::in_tz) don't actually need to hit
/// disk. It will just find a cached copy of a [`TimeZone`] and return that.
///
/// Of course, with caching comes problems of cache invalidation. Invariably,
/// there are parameters that Jiff uses to manage when the cache should be
/// invalidated. Jiff tries to emit log messages about this when it happens. If
/// you find the caching behavior of Jiff to be sub-optimal for your use case,
/// please create an issue. (The plan is likely to expose some options for
/// configuring the behavior of a `TimeZoneDatabase`, but I wanted to collect
/// user feedback first.)
///
/// [IANA Time Zone Database]: https://en.wikipedia.org/wiki/Tz_database
///
/// # Example: list all available time zones
///
/// ```no_run
/// use jiff::tz;
///
/// for tzid in tz::db().available() {
///     println!("{tzid}");
/// }
/// ```
///
/// # Example: using multiple time zone databases
///
/// Jiff supports opening and using multiple time zone databases by default.
/// All you need to do is point [`TimeZoneDatabase::from_dir`] to your own
/// copy of the Time Zone Database, and it will handle the rest.
///
/// This example shows how to utilize multiple databases by parsing a datetime
/// using an older copy of the IANA Time Zone Database. This example leverages
/// the fact that the 2018 copy of the database preceded Brazil's announcement
/// that daylight saving time would be abolished. This meant that datetimes
/// in the future, when parsed with the older copy of the Time Zone Database,
/// would still follow the old daylight saving time rules. But a mere update of
/// the database would otherwise change the meaning of the datetime.
///
/// This scenario can come up if one stores datetimes in the future. This is
/// also why the default offset conflict resolution strategy when parsing zoned
/// datetimes is [`OffsetConflict::Reject`](crate::tz::OffsetConflict::Reject),
/// which prevents one from silently re-interpreting datetimes to a different
/// timestamp.
///
/// ```no_run
/// use jiff::{fmt::temporal::DateTimeParser, tz::{self, TimeZoneDatabase}};
///
/// static PARSER: DateTimeParser = DateTimeParser::new();
///
/// // Open a version of tzdb from before Brazil announced its abolition
/// // of daylight saving time.
/// let tzdb2018 = TimeZoneDatabase::from_dir("path/to/tzdb-2018b")?;
/// // Open the system tzdb.
/// let tzdb = tz::db();
///
/// // Parse the same datetime string with the same parser, but using two
/// // different versions of tzdb.
/// let dt = "2020-01-15T12:00[America/Sao_Paulo]";
/// let zdt2018 = PARSER.parse_zoned_with(&tzdb2018, dt)?;
/// let zdt = PARSER.parse_zoned_with(tzdb, dt)?;
///
/// // Before DST was abolished, 2020-01-15 was in DST, which corresponded
/// // to UTC offset -02. Since DST rules applied to datetimes in the
/// // future, the 2018 version of tzdb would lead one to interpret
/// // 2020-01-15 as being in DST.
/// assert_eq!(zdt2018.offset(), tz::offset(-2));
/// // But DST was abolished in 2019, which means that 2020-01-15 was no
/// // no longer in DST. So after a tzdb update, the same datetime as above
/// // now has a different offset.
/// assert_eq!(zdt.offset(), tz::offset(-3));
///
/// // So if you try to parse a datetime serialized from an older copy of
/// // tzdb, you'll get an error under the default configuration because
/// // of `OffsetConflict::Reject`. This would succeed if you parsed it
/// // using tzdb2018!
/// assert!(PARSER.parse_zoned_with(tzdb, zdt2018.to_string()).is_err());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct TimeZoneDatabase {
    inner: Option<Arc<Kind>>,
}

#[derive(Debug)]
// Needed for core-only "dumb" `Arc`.
#[cfg_attr(not(feature = "alloc"), derive(Clone))]
enum Kind {
    ZoneInfo(zoneinfo::Database),
    Concatenated(concatenated::Database),
    Bundled(bundled::Database),
}

impl TimeZoneDatabase {
    /// Returns a database for which all time zone lookups fail.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz::TimeZoneDatabase;
    ///
    /// let db = TimeZoneDatabase::none();
    /// assert_eq!(db.available().count(), 0);
    /// ```
    pub const fn none() -> TimeZoneDatabase {
        TimeZoneDatabase { inner: None }
    }

    /// Returns a time zone database initialized from the current environment.
    ///
    /// This routine never fails, but it may not be able to find a copy of
    /// your Time Zone Database. When this happens, log messages (with some
    /// at least at the `WARN` level) will be emitted. They can be viewed by
    /// installing a [`log`] compatible logger such as [`env_logger`].
    ///
    /// Typically, one does not need to call this routine directly. Instead,
    /// it's done for you as part of [`jiff::tz::db`](crate::tz::db()).
    /// This does require Jiff's `std` feature to be enabled though. So for
    /// example, you might use this constructor when the features `alloc`
    /// and `tzdb-bundle-always` are enabled to get access to a bundled
    /// copy of the IANA time zone database. (Accessing the system copy at
    /// `/usr/share/zoneinfo` requires `std`.)
    ///
    /// Beware that calling this constructor will create a new _distinct_
    /// handle from the one returned by `jiff::tz::db` with its own cache.
    ///
    /// [`log`]: https://docs.rs/log
    /// [`env_logger`]: https://docs.rs/env_logger
    ///
    /// # Platform behavior
    ///
    /// When the `TZDIR` environment variable is set, this will attempt to
    /// open the Time Zone Database at the directory specified. Otherwise,
    /// this will search a list of predefined directories for a system
    /// installation of the Time Zone Database. Typically, it's found at
    /// `/usr/share/zoneinfo`.
    ///
    /// On Windows systems, under the default crate configuration, this will
    /// return an embedded copy of the Time Zone Database since Windows does
    /// not have a canonical installation of the Time Zone Database.
    pub fn from_env() -> TimeZoneDatabase {
        // On Android, try the concatenated database first, since that's
        // typically what is used.
        //
        // Overall this logic might be sub-optimal. Like, does it really make
        // sense to check for the zoneinfo or concatenated database on non-Unix
        // platforms? Probably not to be honest. But these should only be
        // executed ~once generally, so it doesn't seem like a big deal to try.
        // And trying makes things a little more flexible I think.
        if cfg!(target_os = "android") {
            let db = concatenated::Database::from_env();
            if !db.is_definitively_empty() {
                return TimeZoneDatabase::new(Kind::Concatenated(db));
            }

            let db = zoneinfo::Database::from_env();
            if !db.is_definitively_empty() {
                return TimeZoneDatabase::new(Kind::ZoneInfo(db));
            }
        } else {
            let db = zoneinfo::Database::from_env();
            if !db.is_definitively_empty() {
                return TimeZoneDatabase::new(Kind::ZoneInfo(db));
            }

            let db = concatenated::Database::from_env();
            if !db.is_definitively_empty() {
                return TimeZoneDatabase::new(Kind::Concatenated(db));
            }
        }

        let db = bundled::Database::new();
        if !db.is_definitively_empty() {
            return TimeZoneDatabase::new(Kind::Bundled(db));
        }

        warn!(
            "could not find zoneinfo, concatenated tzdata or \
             bundled time zone database",
        );
        TimeZoneDatabase::none()
    }

    /// Returns a time zone database initialized from the given directory.
    ///
    /// Unlike [`TimeZoneDatabase::from_env`], this always attempts to look for
    /// a copy of the Time Zone Database at the directory given. And if it
    /// fails to find one at that directory, then an error is returned.
    ///
    /// Basically, you should use this when you need to use a _specific_
    /// copy of the Time Zone Database, and use `TimeZoneDatabase::from_env`
    /// when you just want Jiff to try and "do the right thing for you."
    ///
    /// # Errors
    ///
    /// This returns an error if the given directory does not contain a valid
    /// copy of the Time Zone Database. Generally, this means a directory with
    /// at least one valid TZif file.
    #[cfg(feature = "std")]
    pub fn from_dir<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<TimeZoneDatabase, Error> {
        let path = path.as_ref();
        let db = zoneinfo::Database::from_dir(path)?;
        if db.is_definitively_empty() {
            warn!(
                "could not find zoneinfo data at directory {path}",
                path = path.display(),
            );
        }
        Ok(TimeZoneDatabase::new(Kind::ZoneInfo(db)))
    }

    /// Returns a time zone database initialized from a path pointing to a
    /// concatenated `tzdata` file. This type of format is only known to be
    /// found on Android environments. The specific format for this file isn't
    /// defined formally anywhere, but Jiff parses the same format supported
    /// by the [Android Platform].
    ///
    /// Unlike [`TimeZoneDatabase::from_env`], this always attempts to look for
    /// a copy of the Time Zone Database at the path given. And if it
    /// fails to find one at that path, then an error is returned.
    ///
    /// Basically, you should use this when you need to use a _specific_
    /// copy of the Time Zone Database in its concatenated format, and use
    /// `TimeZoneDatabase::from_env` when you just want Jiff to try and "do the
    /// right thing for you." (`TimeZoneDatabase::from_env` will attempt to
    /// automatically detect the presence of a system concatenated `tzdata`
    /// file on Android.)
    ///
    /// # Errors
    ///
    /// This returns an error if the given path does not contain a valid
    /// copy of the concatenated Time Zone Database.
    ///
    /// [Android Platform]: https://android.googlesource.com/platform/libcore/+/jb-mr2-release/luni/src/main/java/libcore/util/ZoneInfoDB.java
    #[cfg(feature = "std")]
    pub fn from_concatenated_path<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<TimeZoneDatabase, Error> {
        let path = path.as_ref();
        let db = concatenated::Database::from_path(path)?;
        if db.is_definitively_empty() {
            warn!(
                "could not find concatenated tzdata in file {path}",
                path = path.display(),
            );
        }
        Ok(TimeZoneDatabase::new(Kind::Concatenated(db)))
    }

    /// Returns a time zone database initialized from the bundled copy of
    /// the [IANA Time Zone Database].
    ///
    /// While this API is always available, in order to get a non-empty
    /// database back, this requires that one of the crate features
    /// `tzdb-bundle-always` or `tzdb-bundle-platform` is enabled. In the
    /// latter case, the bundled database is only available on platforms known
    /// to lack a system copy of the IANA Time Zone Database (i.e., non-Unix
    /// systems).
    ///
    /// This routine is infallible, but it may return a database
    /// that is definitively empty if the bundled data is not
    /// available. To query whether the data is empty or not, use
    /// [`TimeZoneDatabase::is_definitively_empty`].
    ///
    /// # Data generation
    ///
    /// The data in this crate comes from the [IANA Time Zone Database] "data
    /// only" distribution. [`jiff-cli`] is used to first compile the release
    /// into binary TZif data using the `zic` compiler, and secondly, converts
    /// the binary data into a flattened and de-duplicated representation that
    /// is embedded into this crate's source code.
    ///
    /// The conversion into the TZif binary data uses the following settings:
    ///
    /// * The "rearguard" data is used (see below).
    /// * The binary data itself is compiled using the "slim" format. Which
    ///   effectively means that the TZif data primarily only uses explicit
    ///   time zone transitions for historical data and POSIX time zones for
    ///   current time zone transition rules. This doesn't have any impact
    ///   on the actual results. The reason that there are "slim" and "fat"
    ///   formats is to support legacy applications that can't deal with
    ///   POSIX time zones. For example, `/usr/share/zoneinfo` on my modern
    ///   Archlinux installation (2025-02-27) is in the "fat" format.
    ///
    /// The reason that rearguard data is used is a bit more subtle and has
    /// to do with a difference in how the IANA Time Zone Database treats its
    /// internal "daylight saving time" flag and what people in the "real
    /// world" consider "daylight saving time." For example, in the standard
    /// distribution of the IANA Time Zone Database, `Europe/Dublin` has its
    /// daylight saving time flag set to _true_ during Winter and set to
    /// _false_ during Summer. The actual time shifts are the same as, e.g.,
    /// `Europe/London`, but which one is actually labeled "daylight saving
    /// time" is not.
    ///
    /// The IANA Time Zone Database does this for `Europe/Dublin`, presumably,
    /// because _legally_, time during the Summer in Ireland is called `Irish
    /// Standard Time`, and time during the Winter is called `Greenwich Mean
    /// Time`. These legal names are reversed from what is typically the case,
    /// where "standard" time is during the Winter and daylight saving time is
    /// during the Summer. The IANA Time Zone Database implements this tweak in
    /// legal language via a "negative daylight saving time offset." This is
    /// somewhat odd, and some consumers of the IANA Time Zone Database cannot
    /// handle it. Thus, the rearguard format was born for, seemingly, legacy
    /// programs.
    ///
    /// Jiff can handle negative daylight saving time offsets just fine,
    /// but we use the rearguard format anyway so that the underlying data
    /// more accurately reflects on-the-ground reality for humans living in
    /// `Europe/Dublin`. In particular, using the rearguard data enables
    /// [localization of time zone names] to be done correctly.
    ///
    /// [IANA Time Zone Database]: https://en.wikipedia.org/wiki/Tz_database
    /// [`jiff-cli`]: https://github.com/BurntSushi/jiff/tree/master/crates/jiff-cli
    /// [localization of time zone names]: https://github.com/BurntSushi/jiff/issues/258
    pub fn bundled() -> TimeZoneDatabase {
        let db = bundled::Database::new();
        if db.is_definitively_empty() {
            warn!("could not find embedded/bundled zoneinfo");
        }
        TimeZoneDatabase::new(Kind::Bundled(db))
    }

    /// Creates a new DB from the internal kind.
    fn new(kind: Kind) -> TimeZoneDatabase {
        TimeZoneDatabase { inner: Some(Arc::new(kind)) }
    }

    /// Returns a [`TimeZone`] corresponding to the IANA time zone identifier
    /// given.
    ///
    /// The lookup is performed without regard to ASCII case.
    ///
    /// To see a list of all available time zone identifiers for this database,
    /// use [`TimeZoneDatabase::available`].
    ///
    /// It is guaranteed that if the given time zone name is case insensitively
    /// equivalent to `UTC`, then the time zone returned will be equivalent to
    /// `TimeZone::UTC`. Similarly for `Etc/Unknown` and `TimeZone::unknown()`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz;
    ///
    /// let tz = tz::db().get("america/NEW_YORK")?;
    /// assert_eq!(tz.iana_name(), Some("America/New_York"));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get(&self, name: &str) -> Result<TimeZone, Error> {
        let inner = self.inner.as_deref().ok_or_else(|| {
            if cfg!(feature = "std") {
                err!(
                    "failed to find time zone `{name}` since there is no \
                     time zone database configured",
                )
            } else {
                err!(
                    "failed to find time zone `{name}`, there is no \
                     global time zone database configured (and is currently \
                     impossible to do so without Jiff's `std` feature \
                     enabled, if you need this functionality, please file \
                     an issue on Jiff's tracker with your use case)",
                )
            }
        })?;
        match *inner {
            Kind::ZoneInfo(ref db) => {
                if let Some(tz) = db.get(name) {
                    trace!("found time zone `{name}` in {db:?}", db = self);
                    return Ok(tz);
                }
            }
            Kind::Concatenated(ref db) => {
                if let Some(tz) = db.get(name) {
                    trace!("found time zone `{name}` in {db:?}", db = self);
                    return Ok(tz);
                }
            }
            Kind::Bundled(ref db) => {
                if let Some(tz) = db.get(name) {
                    trace!("found time zone `{name}` in {db:?}", db = self);
                    return Ok(tz);
                }
            }
        }
        Err(err!("failed to find time zone `{name}` in time zone database"))
    }

    /// Returns a list of all available time zone identifiers from this
    /// database.
    ///
    /// Note that time zone identifiers are more of a machine readable
    /// abstraction and not an end user level abstraction. Still, users
    /// comfortable with configuring their system's default time zone through
    /// IANA time zone identifiers are probably comfortable interacting with
    /// the identifiers returned here.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use jiff::tz;
    ///
    /// for tzid in tz::db().available() {
    ///     println!("{tzid}");
    /// }
    /// ```
    pub fn available<'d>(&'d self) -> TimeZoneNameIter<'d> {
        let Some(inner) = self.inner.as_deref() else {
            return TimeZoneNameIter::empty();
        };
        match *inner {
            Kind::ZoneInfo(ref db) => db.available(),
            Kind::Concatenated(ref db) => db.available(),
            Kind::Bundled(ref db) => db.available(),
        }
    }

    /// Resets the internal cache of this database.
    ///
    /// Subsequent interactions with this database will need to re-read time
    /// zone data from disk.
    ///
    /// It might be useful to call this if you know the time zone database
    /// has changed on disk and want to force Jiff to re-load it immediately
    /// without spawning a new process or waiting for Jiff's internal cache
    /// invalidation heuristics to kick in.
    pub fn reset(&self) {
        let Some(inner) = self.inner.as_deref() else { return };
        match *inner {
            Kind::ZoneInfo(ref db) => db.reset(),
            Kind::Concatenated(ref db) => db.reset(),
            Kind::Bundled(ref db) => db.reset(),
        }
    }

    /// Returns true if it is known that this time zone database is empty.
    ///
    /// When this returns true, it is guaranteed that all
    /// [`TimeZoneDatabase::get`] calls will fail, and that
    /// [`TimeZoneDatabase::available`] will always return an empty iterator.
    ///
    /// Note that if this returns false, it is still possible for this database
    /// to be empty.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz::TimeZoneDatabase;
    ///
    /// let db = TimeZoneDatabase::none();
    /// assert!(db.is_definitively_empty());
    /// ```
    pub fn is_definitively_empty(&self) -> bool {
        let Some(inner) = self.inner.as_deref() else { return true };
        match *inner {
            Kind::ZoneInfo(ref db) => db.is_definitively_empty(),
            Kind::Concatenated(ref db) => db.is_definitively_empty(),
            Kind::Bundled(ref db) => db.is_definitively_empty(),
        }
    }
}

impl core::fmt::Debug for TimeZoneDatabase {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "TimeZoneDatabase(")?;
        let Some(inner) = self.inner.as_deref() else {
            return write!(f, "unavailable)");
        };
        match *inner {
            Kind::ZoneInfo(ref db) => write!(f, "{db:?}")?,
            Kind::Concatenated(ref db) => write!(f, "{db:?}")?,
            Kind::Bundled(ref db) => write!(f, "{db:?}")?,
        }
        write!(f, ")")
    }
}

/// An iterator over the time zone identifiers in a [`TimeZoneDatabase`].
///
/// This iterator is created by [`TimeZoneDatabase::available`].
///
/// There are no guarantees about the order in which this iterator yields
/// time zone identifiers.
///
/// The lifetime parameter corresponds to the lifetime of the
/// `TimeZoneDatabase` from which this iterator was created.
#[derive(Clone, Debug)]
pub struct TimeZoneNameIter<'d> {
    #[cfg(feature = "alloc")]
    it: alloc::vec::IntoIter<TimeZoneName<'d>>,
    #[cfg(not(feature = "alloc"))]
    it: core::iter::Empty<TimeZoneName<'d>>,
}

impl<'d> TimeZoneNameIter<'d> {
    /// Creates a time zone name iterator that never yields any elements.
    fn empty() -> TimeZoneNameIter<'d> {
        #[cfg(feature = "alloc")]
        {
            TimeZoneNameIter { it: alloc::vec::Vec::new().into_iter() }
        }
        #[cfg(not(feature = "alloc"))]
        {
            TimeZoneNameIter { it: core::iter::empty() }
        }
    }

    /// Creates a time zone name iterator that yields the elements from the
    /// iterator given. (They are collected into a `Vec`.)
    #[cfg(feature = "alloc")]
    fn from_iter(
        it: impl Iterator<Item = impl Into<alloc::string::String>>,
    ) -> TimeZoneNameIter<'d> {
        let names: alloc::vec::Vec<TimeZoneName<'d>> =
            it.map(|name| TimeZoneName::new(name.into())).collect();
        TimeZoneNameIter { it: names.into_iter() }
    }
}

impl<'d> Iterator for TimeZoneNameIter<'d> {
    type Item = TimeZoneName<'d>;

    fn next(&mut self) -> Option<TimeZoneName<'d>> {
        self.it.next()
    }
}

/// A name for a time zone yield by the [`TimeZoneNameIter`] iterator.
///
/// The iterator is created by [`TimeZoneDatabase::available`].
///
/// The lifetime parameter corresponds to the lifetime of the
/// `TimeZoneDatabase` from which this name was created.
#[derive(Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct TimeZoneName<'d> {
    /// The lifetime of the tzdb.
    ///
    /// We don't currently use this, but it could be quite useful if we ever
    /// adopt a "compile time" tzdb like what `chrono-tz` has. Then we could
    /// return strings directly from the embedded data. Or perhaps a "compile
    /// time" TZif or some such.
    lifetime: core::marker::PhantomData<&'d str>,
    #[cfg(feature = "alloc")]
    name: alloc::string::String,
    #[cfg(not(feature = "alloc"))]
    name: core::convert::Infallible,
}

impl<'d> TimeZoneName<'d> {
    /// Returns a new time zone name from the string given.
    ///
    /// The lifetime returned is inferred according to the caller's context.
    #[cfg(feature = "alloc")]
    fn new(name: alloc::string::String) -> TimeZoneName<'d> {
        TimeZoneName { lifetime: core::marker::PhantomData, name }
    }

    /// Returns this time zone name as a borrowed string.
    ///
    /// Note that the lifetime of the string returned is tied to `self`,
    /// which may be shorter than the lifetime `'d` of the originating
    /// `TimeZoneDatabase`.
    #[inline]
    pub fn as_str<'a>(&'a self) -> &'a str {
        #[cfg(feature = "alloc")]
        {
            self.name.as_str()
        }
        #[cfg(not(feature = "alloc"))]
        {
            // Can never be reached because `TimeZoneName` cannot currently
            // be constructed in core-only environments.
            unreachable!()
        }
    }
}

impl<'d> core::fmt::Display for TimeZoneName<'d> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Checks if `name` is a "special" time zone and returns one if so.
///
/// This is limited to special constants that should have consistent values
/// across time zone database implementations. For example, `UTC`.
fn special_time_zone(name: &str) -> Option<TimeZone> {
    if utf8::cmp_ignore_ascii_case("utc", name).is_eq() {
        return Some(TimeZone::UTC);
    }
    if utf8::cmp_ignore_ascii_case("etc/unknown", name).is_eq() {
        return Some(TimeZone::unknown());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This tests that the size of a time zone database is kept at a single
    /// word.
    ///
    /// I think it would probably be okay to make this bigger if we had a
    /// good reason to, but it seems sensible to put a road-block to avoid
    /// accidentally increasing its size.
    #[test]
    fn time_zone_database_size() {
        #[cfg(feature = "alloc")]
        {
            let word = core::mem::size_of::<usize>();
            assert_eq!(word, core::mem::size_of::<TimeZoneDatabase>());
        }
        // A `TimeZoneDatabase` in core-only is vapid.
        #[cfg(not(feature = "alloc"))]
        {
            assert_eq!(1, core::mem::size_of::<TimeZoneDatabase>());
        }
    }

    /// Time zone databases should always return `TimeZone::UTC` if the time
    /// zone is known to be UTC.
    ///
    /// Regression test for: https://github.com/BurntSushi/jiff/issues/346
    #[test]
    fn bundled_returns_utc_constant() {
        let db = TimeZoneDatabase::bundled();
        if db.is_definitively_empty() {
            return;
        }
        assert_eq!(db.get("UTC").unwrap(), TimeZone::UTC);
        assert_eq!(db.get("utc").unwrap(), TimeZone::UTC);
        assert_eq!(db.get("uTc").unwrap(), TimeZone::UTC);
        assert_eq!(db.get("UtC").unwrap(), TimeZone::UTC);

        // Also, similarly, for `Etc/Unknown`.
        assert_eq!(db.get("Etc/Unknown").unwrap(), TimeZone::unknown());
        assert_eq!(db.get("etc/UNKNOWN").unwrap(), TimeZone::unknown());
    }

    /// Time zone databases should always return `TimeZone::UTC` if the time
    /// zone is known to be UTC.
    ///
    /// Regression test for: https://github.com/BurntSushi/jiff/issues/346
    #[cfg(all(feature = "std", not(miri)))]
    #[test]
    fn zoneinfo_returns_utc_constant() {
        let Ok(db) = TimeZoneDatabase::from_dir("/usr/share/zoneinfo") else {
            return;
        };
        if db.is_definitively_empty() {
            return;
        }
        assert_eq!(db.get("UTC").unwrap(), TimeZone::UTC);
        assert_eq!(db.get("utc").unwrap(), TimeZone::UTC);
        assert_eq!(db.get("uTc").unwrap(), TimeZone::UTC);
        assert_eq!(db.get("UtC").unwrap(), TimeZone::UTC);

        // Also, similarly, for `Etc/Unknown`.
        assert_eq!(db.get("Etc/Unknown").unwrap(), TimeZone::unknown());
        assert_eq!(db.get("etc/UNKNOWN").unwrap(), TimeZone::unknown());
    }

    /// This checks that our zoneinfo database never returns a time zone
    /// identifier that isn't presumed to correspond to a real and valid
    /// TZif file in the tzdb.
    ///
    /// This test was added when I optimized the initialized of Jiff's zoneinfo
    /// database. Originally, it did a directory traversal along with a 4-byte
    /// read of every file in the directory to check if the file was TZif or
    /// something else. This turned out to be quite slow on slow file systems.
    /// I rejiggered it so that the reads of every file were removed. But this
    /// meant we could have loaded a name from a file that wasn't TZif into
    /// our in-memory cache.
    ///
    /// For doing a single time zone lookup, this isn't a problem, since we
    /// have to read the TZif data anyway. If it's invalid, then we just
    /// return `None` and log a warning. No big deal.
    ///
    /// But for the `TimeZoneDatabase::available()` API, we were previously
    /// just returning a list of names under the presumption that every such
    /// name corresponds to a valid TZif file. This test checks that we don't
    /// emit junk. (Which was in practice accomplished to moving the 4-byte
    /// read to when we call `TimeZoneDatabase::available()`.)
    ///
    /// Ref: https://github.com/BurntSushi/jiff/issues/366
    #[cfg(all(feature = "std", not(miri)))]
    #[test]
    fn zoneinfo_available_returns_only_tzif() {
        use alloc::{
            collections::BTreeSet,
            string::{String, ToString},
        };

        let Ok(db) = TimeZoneDatabase::from_dir("/usr/share/zoneinfo") else {
            return;
        };
        if db.is_definitively_empty() {
            return;
        }
        let names: BTreeSet<String> =
            db.available().map(|n| n.as_str().to_string()).collect();
        // Not all zoneinfo directories are created equal. Some have more or
        // less junk than others. So just try a few things.
        let should_be_absent = [
            "leapseconds",
            "tzdata.zi",
            "leap-seconds.list",
            "SECURITY",
            "zone1970.tab",
            "iso3166.tab",
            "zonenow.tab",
            "zone.tab",
        ];
        for name in should_be_absent {
            assert!(
                !names.contains(name),
                "found `{name}` in time zone list, but it shouldn't be there",
            );
        }
    }
}
