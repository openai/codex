use crate::tz::{TimeZone, TimeZoneDatabase};

static UNIX_LOCALTIME_PATH: &str = "/etc/localtime";

/// Attempts to find the default "system" time zone.
///
/// In the happy path, this looks at `/etc/localtime`, assumes it is a
/// symlink to something in `/usr/share/zoneinfo` and extracts out the IANA
/// time zone name. From there, it looks up that time zone name in the given
/// time zone database.
///
/// When `/etc/localtime` isn't a symlink, or if the time zone couldn't be
/// found in the given time zone database, then `/etc/localtime` is read
/// directly as a TZif data file describing a time zone. The reason why we
/// try to avoid this is because a TZif data file does not contain the time
/// zone name. And we *really* want the time zone name as it is the only
/// standardized way to roundtrip a datetime in a particular time zone.
pub(super) fn get(db: &TimeZoneDatabase) -> Option<TimeZone> {
    read(db, UNIX_LOCALTIME_PATH)
}

/// Given a path to a system default TZif file, return its corresponding
/// time zone.
///
/// In Unix, we attempt to read it as a symlink and extract an IANA time zone
/// identifier. If that ID exists in the tzdb, we return that. Otherwise, we
/// read the TZif file as an unnamed time zone.
pub(super) fn read(db: &TimeZoneDatabase, path: &str) -> Option<TimeZone> {
    if let Some(tz) = read_link_to_zoneinfo(db, path) {
        return Some(tz);
    }
    trace!(
        "failed to find time zone name using Unix-specific heuristics, \
         attempting to read {UNIX_LOCALTIME_PATH} as unnamed time zone",
    );
    match super::read_unnamed_tzif_file(path) {
        Ok(tz) => Some(tz),
        Err(_err) => {
            trace!("failed to read {path} as unnamed time zone: {_err}");
            None
        }
    }
}

/// Attempt to determine the time zone name from the symlink path given.
///
/// If the path isn't a symlink or if its target wasn't recognized as
/// pointing into a known zoneinfo database, then this returns `None` (and
/// emits some log messages).
fn read_link_to_zoneinfo(
    db: &TimeZoneDatabase,
    path: &str,
) -> Option<TimeZone> {
    let target = match std::fs::read_link(path) {
        Ok(target) => target,
        Err(_err) => {
            trace!("failed to read {path} as symbolic link: {_err}");
            return None;
        }
    };
    let Some(target) = target.to_str() else {
        trace!("symlink target {target:?} for {path:?} is not valid UTF-8");
        return None;
    };
    let needle = "zoneinfo/";
    let Some(rpos) = target.rfind(needle) else {
        trace!(
            "could not find {needle:?} in symlink target {target:?} \
             for path {path:?}, so could not determine time zone name \
             from symlink",
        );
        return None;
    };
    let name = &target[rpos + needle.len()..];
    trace!(
        "extracted {name:?} from symlink target {target:?} \
         for path {path:?} and assuming it is an IANA time zone name",
    );
    let tz = match db.get(&name) {
        Ok(tz) => tz,
        Err(_err) => {
            trace!(
                "using {name:?} symlink target {target:?} \
                 for path {path:?} as time zone name, \
                 but failed to find time zone with that name in \
                 zoneinfo database {db:?}",
            );
            return None;
        }
    };
    Some(tz)
}

#[cfg(not(miri))]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_time_zone_name_etc_localtime() {
        let _ = crate::logging::Logger::init();

        let db = crate::tz::db();
        if crate::tz::db().is_definitively_empty() {
            return;
        }
        let path = std::path::Path::new("/etc/localtime");
        if !path.exists() {
            return;
        }
        // It's hard to assert much other than that a time zone could be
        // successfully constructed. Presumably this may fail in certain
        // environments, but hopefully the `is_definitively_empty` check above
        // will filter most out.
        assert!(get(db).is_some());
    }
}
