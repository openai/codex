//! Types related to a time zone.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::{cmp::Ordering, fmt, str};

use super::rule::{AlternateTime, TransitionRule};
use super::{DAYS_PER_WEEK, Error, SECONDS_PER_DAY, parser};
use crate::NaiveDateTime;

/// Time zone
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct TimeZone {
    /// List of transitions
    transitions: Vec<Transition>,
    /// List of local time types (cannot be empty)
    local_time_types: Vec<LocalTimeType>,
    /// List of leap seconds
    leap_seconds: Vec<LeapSecond>,
    /// Extra transition rule applicable after the last transition
    extra_rule: Option<TransitionRule>,
}

impl TimeZone {
    /// Returns local time zone.
    ///
    /// This method in not supported on non-UNIX platforms, and returns the UTC time zone instead.
    pub(crate) fn local(env_tz: Option<&str>) -> Result<Self, Error> {
        match env_tz {
            Some(tz) => Self::from_posix_tz(tz),
            None => Self::from_posix_tz("localtime"),
        }
    }

    /// Construct a time zone from a POSIX TZ string, as described in [the POSIX documentation of the `TZ` environment variable](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap08.html).
    fn from_posix_tz(tz_string: &str) -> Result<Self, Error> {
        // It is commonly agreed (but not standard) that setting an empty `TZ=` uses UTC.
        if tz_string.is_empty() {
            return Ok(Self::utc());
        }

        if tz_string == "localtime" {
            return Self::from_tz_data(&fs::read("/etc/localtime")?);
        }

        // attributes are not allowed on if blocks in Rust 1.38
        #[cfg(any(target_os = "android", target_env = "ohos"))]
        {
            if let Ok(Some(bytes)) = crate::offset::local::tz_data::for_zone(tz_string) {
                return Self::from_tz_data(&bytes);
            }
        }

        let mut chars = tz_string.chars();
        if chars.next() == Some(':') {
            return Self::from_file(&mut find_tz_file(chars.as_str())?);
        }

        if let Ok(mut file) = find_tz_file(tz_string) {
            return Self::from_file(&mut file);
        }

        // TZ string extensions are not allowed
        let tz_string = tz_string.trim_matches(|c: char| c.is_ascii_whitespace());
        let rule = TransitionRule::from_tz_string(tz_string.as_bytes(), false)?;
        Self::new(
            vec![],
            match rule {
                TransitionRule::Fixed(local_time_type) => vec![local_time_type],
                TransitionRule::Alternate(AlternateTime { std, dst, .. }) => vec![std, dst],
            },
            vec![],
            Some(rule),
        )
    }

    /// Construct a time zone
    pub(super) fn new(
        transitions: Vec<Transition>,
        local_time_types: Vec<LocalTimeType>,
        leap_seconds: Vec<LeapSecond>,
        extra_rule: Option<TransitionRule>,
    ) -> Result<Self, Error> {
        let new = Self { transitions, local_time_types, leap_seconds, extra_rule };
        new.as_ref().validate()?;
        Ok(new)
    }

    /// Construct a time zone from the contents of a time zone file
    fn from_file(file: &mut File) -> Result<Self, Error> {
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Self::from_tz_data(&bytes)
    }

    /// Construct a time zone from the contents of a time zone file
    ///
    /// Parse TZif data as described in [RFC 8536](https://datatracker.ietf.org/doc/html/rfc8536).
    pub(crate) fn from_tz_data(bytes: &[u8]) -> Result<Self, Error> {
        parser::parse(bytes)
    }

    /// Construct a time zone with the specified UTC offset in seconds
    fn fixed(ut_offset: i32) -> Result<Self, Error> {
        Ok(Self {
            transitions: Vec::new(),
            local_time_types: vec![LocalTimeType::with_offset(ut_offset)?],
            leap_seconds: Vec::new(),
            extra_rule: None,
        })
    }

    /// Construct the time zone associated to UTC
    pub(crate) fn utc() -> Self {
        Self {
            transitions: Vec::new(),
            local_time_types: vec![LocalTimeType::UTC],
            leap_seconds: Vec::new(),
            extra_rule: None,
        }
    }

    /// Find the local time type associated to the time zone at the specified Unix time in seconds
    pub(crate) fn find_local_time_type(&self, unix_time: i64) -> Result<&LocalTimeType, Error> {
        self.as_ref().find_local_time_type(unix_time)
    }

    pub(crate) fn find_local_time_type_from_local(
        &self,
        local_time: NaiveDateTime,
    ) -> Result<crate::MappedLocalTime<LocalTimeType>, Error> {
        self.as_ref().find_local_time_type_from_local(local_time)
    }

    /// Returns a reference to the time zone
    fn as_ref(&self) -> TimeZoneRef<'_> {
        TimeZoneRef {
            transitions: &self.transitions,
            local_time_types: &self.local_time_types,
            leap_seconds: &self.leap_seconds,
            extra_rule: &self.extra_rule,
        }
    }
}

/// Reference to a time zone
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct TimeZoneRef<'a> {
    /// List of transitions
    transitions: &'a [Transition],
    /// List of local time types (cannot be empty)
    local_time_types: &'a [LocalTimeType],
    /// List of leap seconds
    leap_seconds: &'a [LeapSecond],
    /// Extra transition rule applicable after the last transition
    extra_rule: &'a Option<TransitionRule>,
}

impl<'a> TimeZoneRef<'a> {
    /// Find the local time type associated to the time zone at the specified Unix time in seconds
    pub(crate) fn find_local_time_type(&self, unix_time: i64) -> Result<&'a LocalTimeType, Error> {
        let extra_rule = match self.transitions.last() {
            None => match self.extra_rule {
                Some(extra_rule) => extra_rule,
                None => return Ok(&self.local_time_types[0]),
            },
            Some(last_transition) => {
                let unix_leap_time = match self.unix_time_to_unix_leap_time(unix_time) {
                    Ok(unix_leap_time) => unix_leap_time,
                    Err(Error::OutOfRange(error)) => return Err(Error::FindLocalTimeType(error)),
                    Err(err) => return Err(err),
                };

                if unix_leap_time >= last_transition.unix_leap_time {
                    match self.extra_rule {
                        Some(extra_rule) => extra_rule,
                        None => {
                            // RFC 8536 3.2:
                            // "Local time for timestamps on or after the last transition is
                            // specified by the TZ string in the footer (Section 3.3) if present
                            // and nonempty; otherwise, it is unspecified."
                            //
                            // Older versions of macOS (1.12 and before?) have TZif file with a
                            // missing TZ string, and use the offset given by the last transition.
                            return Ok(
                                &self.local_time_types[last_transition.local_time_type_index]
                            );
                        }
                    }
                } else {
                    let index = match self
                        .transitions
                        .binary_search_by_key(&unix_leap_time, Transition::unix_leap_time)
                    {
                        Ok(x) => x + 1,
                        Err(x) => x,
                    };

                    let local_time_type_index = if index > 0 {
                        self.transitions[index - 1].local_time_type_index
                    } else {
                        0
                    };
                    return Ok(&self.local_time_types[local_time_type_index]);
                }
            }
        };

        match extra_rule.find_local_time_type(unix_time) {
            Ok(local_time_type) => Ok(local_time_type),
            Err(Error::OutOfRange(error)) => Err(Error::FindLocalTimeType(error)),
            err => err,
        }
    }

    pub(crate) fn find_local_time_type_from_local(
        &self,
        local_time: NaiveDateTime,
    ) -> Result<crate::MappedLocalTime<LocalTimeType>, Error> {
        // #TODO: this is wrong as we need 'local_time_to_local_leap_time ?
        // but ... does the local time even include leap seconds ??
        // let unix_leap_time = match self.unix_time_to_unix_leap_time(local_time) {
        //     Ok(unix_leap_time) => unix_leap_time,
        //     Err(Error::OutOfRange(error)) => return Err(Error::FindLocalTimeType(error)),
        //     Err(err) => return Err(err),
        // };
        let local_leap_time = local_time.and_utc().timestamp();

        // if we have at least one transition,
        // we must check _all_ of them, in case of any Overlapping (MappedLocalTime::Ambiguous) or Skipping (MappedLocalTime::None) transitions
        let offset_after_last = if !self.transitions.is_empty() {
            let mut prev = self.local_time_types[0];

            for transition in self.transitions {
                let after_ltt = self.local_time_types[transition.local_time_type_index];

                // the end and start here refers to where the time starts prior to the transition
                // and where it ends up after. not the temporal relationship.
                let transition_end = transition.unix_leap_time + i64::from(after_ltt.ut_offset);
                let transition_start = transition.unix_leap_time + i64::from(prev.ut_offset);

                match transition_start.cmp(&transition_end) {
                    Ordering::Greater => {
                        // backwards transition, eg from DST to regular
                        // this means a given local time could have one of two possible offsets
                        if local_leap_time < transition_end {
                            return Ok(crate::MappedLocalTime::Single(prev));
                        } else if local_leap_time >= transition_end
                            && local_leap_time <= transition_start
                        {
                            if prev.ut_offset < after_ltt.ut_offset {
                                return Ok(crate::MappedLocalTime::Ambiguous(prev, after_ltt));
                            } else {
                                return Ok(crate::MappedLocalTime::Ambiguous(after_ltt, prev));
                            }
                        }
                    }
                    Ordering::Equal => {
                        // should this ever happen? presumably we have to handle it anyway.
                        if local_leap_time < transition_start {
                            return Ok(crate::MappedLocalTime::Single(prev));
                        } else if local_leap_time == transition_end {
                            if prev.ut_offset < after_ltt.ut_offset {
                                return Ok(crate::MappedLocalTime::Ambiguous(prev, after_ltt));
                            } else {
                                return Ok(crate::MappedLocalTime::Ambiguous(after_ltt, prev));
                            }
                        }
                    }
                    Ordering::Less => {
                        // forwards transition, eg from regular to DST
                        // this means that times that are skipped are invalid local times
                        if local_leap_time <= transition_start {
                            return Ok(crate::MappedLocalTime::Single(prev));
                        } else if local_leap_time < transition_end {
                            return Ok(crate::MappedLocalTime::None);
                        } else if local_leap_time == transition_end {
                            return Ok(crate::MappedLocalTime::Single(after_ltt));
                        }
                    }
                }

                // try the next transition, we are fully after this one
                prev = after_ltt;
            }

            prev
        } else {
            self.local_time_types[0]
        };

        if let Some(extra_rule) = self.extra_rule {
            match extra_rule.find_local_time_type_from_local(local_time) {
                Ok(local_time_type) => Ok(local_time_type),
                Err(Error::OutOfRange(error)) => Err(Error::FindLocalTimeType(error)),
                err => err,
            }
        } else {
            Ok(crate::MappedLocalTime::Single(offset_after_last))
        }
    }

    /// Check time zone inputs
    fn validate(&self) -> Result<(), Error> {
        // Check local time types
        let local_time_types_size = self.local_time_types.len();
        if local_time_types_size == 0 {
            return Err(Error::TimeZone("list of local time types must not be empty"));
        }

        // Check transitions
        let mut i_transition = 0;
        while i_transition < self.transitions.len() {
            if self.transitions[i_transition].local_time_type_index >= local_time_types_size {
                return Err(Error::TimeZone("invalid local time type index"));
            }

            if i_transition + 1 < self.transitions.len()
                && self.transitions[i_transition].unix_leap_time
                    >= self.transitions[i_transition + 1].unix_leap_time
            {
                return Err(Error::TimeZone("invalid transition"));
            }

            i_transition += 1;
        }

        // Check leap seconds
        if !(self.leap_seconds.is_empty()
            || self.leap_seconds[0].unix_leap_time >= 0
                && self.leap_seconds[0].correction.saturating_abs() == 1)
        {
            return Err(Error::TimeZone("invalid leap second"));
        }

        let min_interval = SECONDS_PER_28_DAYS - 1;

        let mut i_leap_second = 0;
        while i_leap_second < self.leap_seconds.len() {
            if i_leap_second + 1 < self.leap_seconds.len() {
                let x0 = &self.leap_seconds[i_leap_second];
                let x1 = &self.leap_seconds[i_leap_second + 1];

                let diff_unix_leap_time = x1.unix_leap_time.saturating_sub(x0.unix_leap_time);
                let abs_diff_correction =
                    x1.correction.saturating_sub(x0.correction).saturating_abs();

                if !(diff_unix_leap_time >= min_interval && abs_diff_correction == 1) {
                    return Err(Error::TimeZone("invalid leap second"));
                }
            }
            i_leap_second += 1;
        }

        // Check extra rule
        let (extra_rule, last_transition) = match (&self.extra_rule, self.transitions.last()) {
            (Some(rule), Some(trans)) => (rule, trans),
            _ => return Ok(()),
        };

        let last_local_time_type = &self.local_time_types[last_transition.local_time_type_index];
        let unix_time = match self.unix_leap_time_to_unix_time(last_transition.unix_leap_time) {
            Ok(unix_time) => unix_time,
            Err(Error::OutOfRange(error)) => return Err(Error::TimeZone(error)),
            Err(err) => return Err(err),
        };

        let rule_local_time_type = match extra_rule.find_local_time_type(unix_time) {
            Ok(rule_local_time_type) => rule_local_time_type,
            Err(Error::OutOfRange(error)) => return Err(Error::TimeZone(error)),
            Err(err) => return Err(err),
        };

        let check = last_local_time_type.ut_offset == rule_local_time_type.ut_offset
            && last_local_time_type.is_dst == rule_local_time_type.is_dst
            && match (&last_local_time_type.name, &rule_local_time_type.name) {
                (Some(x), Some(y)) => x.equal(y),
                (None, None) => true,
                _ => false,
            };

        if !check {
            return Err(Error::TimeZone(
                "extra transition rule is inconsistent with the last transition",
            ));
        }

        Ok(())
    }

    /// Convert Unix time to Unix leap time, from the list of leap seconds in a time zone
    const fn unix_time_to_unix_leap_time(&self, unix_time: i64) -> Result<i64, Error> {
        let mut unix_leap_time = unix_time;

        let mut i = 0;
        while i < self.leap_seconds.len() {
            let leap_second = &self.leap_seconds[i];

            if unix_leap_time < leap_second.unix_leap_time {
                break;
            }

            unix_leap_time = match unix_time.checked_add(leap_second.correction as i64) {
                Some(unix_leap_time) => unix_leap_time,
                None => return Err(Error::OutOfRange("out of range operation")),
            };

            i += 1;
        }

        Ok(unix_leap_time)
    }

    /// Convert Unix leap time to Unix time, from the list of leap seconds in a time zone
    fn unix_leap_time_to_unix_time(&self, unix_leap_time: i64) -> Result<i64, Error> {
        if unix_leap_time == i64::MIN {
            return Err(Error::OutOfRange("out of range operation"));
        }

        let index = match self
            .leap_seconds
            .binary_search_by_key(&(unix_leap_time - 1), LeapSecond::unix_leap_time)
        {
            Ok(x) => x + 1,
            Err(x) => x,
        };

        let correction = if index > 0 { self.leap_seconds[index - 1].correction } else { 0 };

        match unix_leap_time.checked_sub(correction as i64) {
            Some(unix_time) => Ok(unix_time),
            None => Err(Error::OutOfRange("out of range operation")),
        }
    }

    /// The UTC time zone
    const UTC: TimeZoneRef<'static> = TimeZoneRef {
        transitions: &[],
        local_time_types: &[LocalTimeType::UTC],
        leap_seconds: &[],
        extra_rule: &None,
    };
}

/// Transition of a TZif file
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) struct Transition {
    /// Unix leap time
    unix_leap_time: i64,
    /// Index specifying the local time type of the transition
    local_time_type_index: usize,
}

impl Transition {
    /// Construct a TZif file transition
    pub(super) const fn new(unix_leap_time: i64, local_time_type_index: usize) -> Self {
        Self { unix_leap_time, local_time_type_index }
    }

    /// Returns Unix leap time
    const fn unix_leap_time(&self) -> i64 {
        self.unix_leap_time
    }
}

/// Leap second of a TZif file
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) struct LeapSecond {
    /// Unix leap time
    unix_leap_time: i64,
    /// Leap second correction
    correction: i32,
}

impl LeapSecond {
    /// Construct a TZif file leap second
    pub(super) const fn new(unix_leap_time: i64, correction: i32) -> Self {
        Self { unix_leap_time, correction }
    }

    /// Returns Unix leap time
    const fn unix_leap_time(&self) -> i64 {
        self.unix_leap_time
    }
}

/// ASCII-encoded fixed-capacity string, used for storing time zone names
#[derive(Copy, Clone, Eq, PartialEq)]
struct TimeZoneName {
    /// Length-prefixed string buffer
    bytes: [u8; 8],
}

impl TimeZoneName {
    /// Construct a time zone name
    ///
    /// man tzfile(5):
    /// Time zone designations should consist of at least three (3) and no more than six (6) ASCII
    /// characters from the set of alphanumerics, “-”, and “+”. This is for compatibility with
    /// POSIX requirements for time zone abbreviations.
    fn new(input: &[u8]) -> Result<Self, Error> {
        let len = input.len();

        if !(3..=7).contains(&len) {
            return Err(Error::LocalTimeType(
                "time zone name must have between 3 and 7 characters",
            ));
        }

        let mut bytes = [0; 8];
        bytes[0] = input.len() as u8;

        let mut i = 0;
        while i < len {
            let b = input[i];
            match b {
                b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'+' | b'-' => {}
                _ => return Err(Error::LocalTimeType("invalid characters in time zone name")),
            }

            bytes[i + 1] = b;
            i += 1;
        }

        Ok(Self { bytes })
    }

    /// Returns time zone name as a byte slice
    fn as_bytes(&self) -> &[u8] {
        match self.bytes[0] {
            3 => &self.bytes[1..4],
            4 => &self.bytes[1..5],
            5 => &self.bytes[1..6],
            6 => &self.bytes[1..7],
            7 => &self.bytes[1..8],
            _ => unreachable!(),
        }
    }

    /// Check if two time zone names are equal
    fn equal(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl AsRef<str> for TimeZoneName {
    fn as_ref(&self) -> &str {
        // SAFETY: ASCII is valid UTF-8
        unsafe { str::from_utf8_unchecked(self.as_bytes()) }
    }
}

impl fmt::Debug for TimeZoneName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

/// Local time type associated to a time zone
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct LocalTimeType {
    /// Offset from UTC in seconds
    pub(super) ut_offset: i32,
    /// Daylight Saving Time indicator
    is_dst: bool,
    /// Time zone name
    name: Option<TimeZoneName>,
}

impl LocalTimeType {
    /// Construct a local time type
    pub(super) fn new(ut_offset: i32, is_dst: bool, name: Option<&[u8]>) -> Result<Self, Error> {
        if ut_offset == i32::MIN {
            return Err(Error::LocalTimeType("invalid UTC offset"));
        }

        let name = match name {
            Some(name) => TimeZoneName::new(name)?,
            None => return Ok(Self { ut_offset, is_dst, name: None }),
        };

        Ok(Self { ut_offset, is_dst, name: Some(name) })
    }

    /// Construct a local time type with the specified UTC offset in seconds
    pub(super) const fn with_offset(ut_offset: i32) -> Result<Self, Error> {
        if ut_offset == i32::MIN {
            return Err(Error::LocalTimeType("invalid UTC offset"));
        }

        Ok(Self { ut_offset, is_dst: false, name: None })
    }

    /// Returns offset from UTC in seconds
    pub(crate) const fn offset(&self) -> i32 {
        self.ut_offset
    }

    /// Returns daylight saving time indicator
    pub(super) const fn is_dst(&self) -> bool {
        self.is_dst
    }

    pub(super) const UTC: LocalTimeType = Self { ut_offset: 0, is_dst: false, name: None };
}

/// Open the TZif file corresponding to a TZ string
fn find_tz_file(path: impl AsRef<Path>) -> Result<File, Error> {
    // Don't check system timezone directories on non-UNIX platforms
    #[cfg(not(unix))]
    return Ok(File::open(path)?);

    #[cfg(unix)]
    {
        let path = path.as_ref();
        if path.is_absolute() {
            return Ok(File::open(path)?);
        }

        for folder in &ZONE_INFO_DIRECTORIES {
            if let Ok(file) = File::open(PathBuf::from(folder).join(path)) {
                return Ok(file);
            }
        }

        Err(Error::Io(io::ErrorKind::NotFound.into()))
    }
}

// Possible system timezone directories
#[cfg(unix)]
const ZONE_INFO_DIRECTORIES: [&str; 4] =
    ["/usr/share/zoneinfo", "/share/zoneinfo", "/etc/zoneinfo", "/usr/share/lib/zoneinfo"];

/// Number of seconds in one week
pub(crate) const SECONDS_PER_WEEK: i64 = SECONDS_PER_DAY * DAYS_PER_WEEK;
/// Number of seconds in 28 days
const SECONDS_PER_28_DAYS: i64 = SECONDS_PER_DAY * 28;

#[cfg(test)]
mod tests {
    use super::super::Error;
    use super::{LeapSecond, LocalTimeType, TimeZone, TimeZoneName, Transition, TransitionRule};

    #[test]
    fn test_no_dst() -> Result<(), Error> {
        let tz_string = b"HST10";
        let transition_rule = TransitionRule::from_tz_string(tz_string, false)?;
        assert_eq!(transition_rule, LocalTimeType::new(-36000, false, Some(b"HST"))?.into());
        Ok(())
    }

    #[test]
    fn test_error() -> Result<(), Error> {
        assert!(matches!(
            TransitionRule::from_tz_string(b"IST-1GMT0", false),
            Err(Error::UnsupportedTzString(_))
        ));
        assert!(matches!(
            TransitionRule::from_tz_string(b"EET-2EEST", false),
            Err(Error::UnsupportedTzString(_))
        ));

        Ok(())
    }

    #[test]
    fn test_v1_file_with_leap_seconds() -> Result<(), Error> {
        let bytes = b"TZif\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\x01\0\0\0\x1b\0\0\0\0\0\0\0\x01\0\0\0\x04\0\0\0\0\0\0UTC\0\x04\xb2\x58\0\0\0\0\x01\x05\xa4\xec\x01\0\0\0\x02\x07\x86\x1f\x82\0\0\0\x03\x09\x67\x53\x03\0\0\0\x04\x0b\x48\x86\x84\0\0\0\x05\x0d\x2b\x0b\x85\0\0\0\x06\x0f\x0c\x3f\x06\0\0\0\x07\x10\xed\x72\x87\0\0\0\x08\x12\xce\xa6\x08\0\0\0\x09\x15\x9f\xca\x89\0\0\0\x0a\x17\x80\xfe\x0a\0\0\0\x0b\x19\x62\x31\x8b\0\0\0\x0c\x1d\x25\xea\x0c\0\0\0\x0d\x21\xda\xe5\x0d\0\0\0\x0e\x25\x9e\x9d\x8e\0\0\0\x0f\x27\x7f\xd1\x0f\0\0\0\x10\x2a\x50\xf5\x90\0\0\0\x11\x2c\x32\x29\x11\0\0\0\x12\x2e\x13\x5c\x92\0\0\0\x13\x30\xe7\x24\x13\0\0\0\x14\x33\xb8\x48\x94\0\0\0\x15\x36\x8c\x10\x15\0\0\0\x16\x43\xb7\x1b\x96\0\0\0\x17\x49\x5c\x07\x97\0\0\0\x18\x4f\xef\x93\x18\0\0\0\x19\x55\x93\x2d\x99\0\0\0\x1a\x58\x68\x46\x9a\0\0\0\x1b\0\0";

        let time_zone = TimeZone::from_tz_data(bytes)?;

        let time_zone_result = TimeZone::new(
            Vec::new(),
            vec![LocalTimeType::new(0, false, Some(b"UTC"))?],
            vec![
                LeapSecond::new(78796800, 1),
                LeapSecond::new(94694401, 2),
                LeapSecond::new(126230402, 3),
                LeapSecond::new(157766403, 4),
                LeapSecond::new(189302404, 5),
                LeapSecond::new(220924805, 6),
                LeapSecond::new(252460806, 7),
                LeapSecond::new(283996807, 8),
                LeapSecond::new(315532808, 9),
                LeapSecond::new(362793609, 10),
                LeapSecond::new(394329610, 11),
                LeapSecond::new(425865611, 12),
                LeapSecond::new(489024012, 13),
                LeapSecond::new(567993613, 14),
                LeapSecond::new(631152014, 15),
                LeapSecond::new(662688015, 16),
                LeapSecond::new(709948816, 17),
                LeapSecond::new(741484817, 18),
                LeapSecond::new(773020818, 19),
                LeapSecond::new(820454419, 20),
                LeapSecond::new(867715220, 21),
                LeapSecond::new(915148821, 22),
                LeapSecond::new(1136073622, 23),
                LeapSecond::new(1230768023, 24),
                LeapSecond::new(1341100824, 25),
                LeapSecond::new(1435708825, 26),
                LeapSecond::new(1483228826, 27),
            ],
            None,
        )?;

        assert_eq!(time_zone, time_zone_result);

        Ok(())
    }

    #[test]
    fn test_v2_file() -> Result<(), Error> {
        let bytes = b"TZif2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x06\0\0\0\x06\0\0\0\0\0\0\0\x07\0\0\0\x06\0\0\0\x14\x80\0\0\0\xbb\x05\x43\x48\xbb\x21\x71\x58\xcb\x89\x3d\xc8\xd2\x23\xf4\x70\xd2\x61\x49\x38\xd5\x8d\x73\x48\x01\x02\x01\x03\x04\x01\x05\xff\xff\x6c\x02\0\0\xff\xff\x6c\x58\0\x04\xff\xff\x7a\x68\x01\x08\xff\xff\x7a\x68\x01\x0c\xff\xff\x7a\x68\x01\x10\xff\xff\x73\x60\0\x04LMT\0HST\0HDT\0HWT\0HPT\0\0\0\0\0\x01\0\0\0\0\0\x01\0TZif2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x06\0\0\0\x06\0\0\0\0\0\0\0\x07\0\0\0\x06\0\0\0\x14\xff\xff\xff\xff\x74\xe0\x70\xbe\xff\xff\xff\xff\xbb\x05\x43\x48\xff\xff\xff\xff\xbb\x21\x71\x58\xff\xff\xff\xff\xcb\x89\x3d\xc8\xff\xff\xff\xff\xd2\x23\xf4\x70\xff\xff\xff\xff\xd2\x61\x49\x38\xff\xff\xff\xff\xd5\x8d\x73\x48\x01\x02\x01\x03\x04\x01\x05\xff\xff\x6c\x02\0\0\xff\xff\x6c\x58\0\x04\xff\xff\x7a\x68\x01\x08\xff\xff\x7a\x68\x01\x0c\xff\xff\x7a\x68\x01\x10\xff\xff\x73\x60\0\x04LMT\0HST\0HDT\0HWT\0HPT\0\0\0\0\0\x01\0\0\0\0\0\x01\0\x0aHST10\x0a";

        let time_zone = TimeZone::from_tz_data(bytes)?;

        let time_zone_result = TimeZone::new(
            vec![
                Transition::new(-2334101314, 1),
                Transition::new(-1157283000, 2),
                Transition::new(-1155436200, 1),
                Transition::new(-880198200, 3),
                Transition::new(-769395600, 4),
                Transition::new(-765376200, 1),
                Transition::new(-712150200, 5),
            ],
            vec![
                LocalTimeType::new(-37886, false, Some(b"LMT"))?,
                LocalTimeType::new(-37800, false, Some(b"HST"))?,
                LocalTimeType::new(-34200, true, Some(b"HDT"))?,
                LocalTimeType::new(-34200, true, Some(b"HWT"))?,
                LocalTimeType::new(-34200, true, Some(b"HPT"))?,
                LocalTimeType::new(-36000, false, Some(b"HST"))?,
            ],
            Vec::new(),
            Some(TransitionRule::from(LocalTimeType::new(-36000, false, Some(b"HST"))?)),
        )?;

        assert_eq!(time_zone, time_zone_result);

        assert_eq!(
            *time_zone.find_local_time_type(-1156939200)?,
            LocalTimeType::new(-34200, true, Some(b"HDT"))?
        );
        assert_eq!(
            *time_zone.find_local_time_type(1546300800)?,
            LocalTimeType::new(-36000, false, Some(b"HST"))?
        );

        Ok(())
    }

    #[test]
    fn test_no_tz_string() -> Result<(), Error> {
        // Guayaquil from macOS 10.11
        let bytes = b"TZif\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x02\0\0\0\x02\0\0\0\0\0\0\0\x01\0\0\0\x02\0\0\0\x08\xb6\xa4B\x18\x01\xff\xff\xb6h\0\0\xff\xff\xb9\xb0\0\x04QMT\0ECT\0\0\0\0\0";

        let time_zone = TimeZone::from_tz_data(bytes)?;
        dbg!(&time_zone);

        let time_zone_result = TimeZone::new(
            vec![Transition::new(-1230749160, 1)],
            vec![
                LocalTimeType::new(-18840, false, Some(b"QMT"))?,
                LocalTimeType::new(-18000, false, Some(b"ECT"))?,
            ],
            Vec::new(),
            None,
        )?;

        assert_eq!(time_zone, time_zone_result);

        assert_eq!(
            *time_zone.find_local_time_type(-1500000000)?,
            LocalTimeType::new(-18840, false, Some(b"QMT"))?
        );
        assert_eq!(
            *time_zone.find_local_time_type(0)?,
            LocalTimeType::new(-18000, false, Some(b"ECT"))?
        );

        Ok(())
    }

    #[test]
    fn test_tz_ascii_str() -> Result<(), Error> {
        assert!(matches!(TimeZoneName::new(b""), Err(Error::LocalTimeType(_))));
        assert!(matches!(TimeZoneName::new(b"A"), Err(Error::LocalTimeType(_))));
        assert!(matches!(TimeZoneName::new(b"AB"), Err(Error::LocalTimeType(_))));
        assert_eq!(TimeZoneName::new(b"CET")?.as_bytes(), b"CET");
        assert_eq!(TimeZoneName::new(b"CHADT")?.as_bytes(), b"CHADT");
        assert_eq!(TimeZoneName::new(b"abcdefg")?.as_bytes(), b"abcdefg");
        assert_eq!(TimeZoneName::new(b"UTC+02")?.as_bytes(), b"UTC+02");
        assert_eq!(TimeZoneName::new(b"-1230")?.as_bytes(), b"-1230");
        assert!(matches!(TimeZoneName::new("−0330".as_bytes()), Err(Error::LocalTimeType(_)))); // MINUS SIGN (U+2212)
        assert!(matches!(TimeZoneName::new(b"\x00123"), Err(Error::LocalTimeType(_))));
        assert!(matches!(TimeZoneName::new(b"12345678"), Err(Error::LocalTimeType(_))));
        assert!(matches!(TimeZoneName::new(b"GMT\0\0\0"), Err(Error::LocalTimeType(_))));

        Ok(())
    }

    #[test]
    fn test_time_zone() -> Result<(), Error> {
        let utc = LocalTimeType::UTC;
        let cet = LocalTimeType::with_offset(3600)?;

        let utc_local_time_types = vec![utc];
        let fixed_extra_rule = TransitionRule::from(cet);

        let time_zone_1 = TimeZone::new(vec![], utc_local_time_types.clone(), vec![], None)?;
        let time_zone_2 =
            TimeZone::new(vec![], utc_local_time_types.clone(), vec![], Some(fixed_extra_rule))?;
        let time_zone_3 =
            TimeZone::new(vec![Transition::new(0, 0)], utc_local_time_types.clone(), vec![], None)?;
        let time_zone_4 = TimeZone::new(
            vec![Transition::new(i32::MIN.into(), 0), Transition::new(0, 1)],
            vec![utc, cet],
            Vec::new(),
            Some(fixed_extra_rule),
        )?;

        assert_eq!(*time_zone_1.find_local_time_type(0)?, utc);
        assert_eq!(*time_zone_2.find_local_time_type(0)?, cet);

        assert_eq!(*time_zone_3.find_local_time_type(-1)?, utc);
        assert_eq!(*time_zone_3.find_local_time_type(0)?, utc);

        assert_eq!(*time_zone_4.find_local_time_type(-1)?, utc);
        assert_eq!(*time_zone_4.find_local_time_type(0)?, cet);

        let time_zone_err = TimeZone::new(
            vec![Transition::new(0, 0)],
            utc_local_time_types,
            vec![],
            Some(fixed_extra_rule),
        );
        assert!(time_zone_err.is_err());

        Ok(())
    }

    #[test]
    fn test_time_zone_from_posix_tz() -> Result<(), Error> {
        #[cfg(unix)]
        {
            // if the TZ var is set, this essentially _overrides_ the
            // time set by the localtime symlink
            // so just ensure that ::local() acts as expected
            // in this case
            if let Ok(tz) = std::env::var("TZ") {
                let time_zone_local = TimeZone::local(Some(tz.as_str()))?;
                let time_zone_local_1 = TimeZone::from_posix_tz(&tz)?;
                assert_eq!(time_zone_local, time_zone_local_1);
            }

            // `TimeZone::from_posix_tz("UTC")` will return `Error` if the environment does not have
            // a time zone database, like for example some docker containers.
            // In that case skip the test.
            if let Ok(time_zone_utc) = TimeZone::from_posix_tz("UTC") {
                assert_eq!(time_zone_utc.find_local_time_type(0)?.offset(), 0);
            }
        }

        assert!(TimeZone::from_posix_tz("EST5EDT,0/0,J365/25").is_err());
        assert_eq!(TimeZone::from_posix_tz("").unwrap().find_local_time_type(0)?.offset(), 0);

        Ok(())
    }

    #[test]
    fn test_leap_seconds() -> Result<(), Error> {
        let time_zone = TimeZone::new(
            Vec::new(),
            vec![LocalTimeType::new(0, false, Some(b"UTC"))?],
            vec![
                LeapSecond::new(78796800, 1),
                LeapSecond::new(94694401, 2),
                LeapSecond::new(126230402, 3),
                LeapSecond::new(157766403, 4),
                LeapSecond::new(189302404, 5),
                LeapSecond::new(220924805, 6),
                LeapSecond::new(252460806, 7),
                LeapSecond::new(283996807, 8),
                LeapSecond::new(315532808, 9),
                LeapSecond::new(362793609, 10),
                LeapSecond::new(394329610, 11),
                LeapSecond::new(425865611, 12),
                LeapSecond::new(489024012, 13),
                LeapSecond::new(567993613, 14),
                LeapSecond::new(631152014, 15),
                LeapSecond::new(662688015, 16),
                LeapSecond::new(709948816, 17),
                LeapSecond::new(741484817, 18),
                LeapSecond::new(773020818, 19),
                LeapSecond::new(820454419, 20),
                LeapSecond::new(867715220, 21),
                LeapSecond::new(915148821, 22),
                LeapSecond::new(1136073622, 23),
                LeapSecond::new(1230768023, 24),
                LeapSecond::new(1341100824, 25),
                LeapSecond::new(1435708825, 26),
                LeapSecond::new(1483228826, 27),
            ],
            None,
        )?;

        let time_zone_ref = time_zone.as_ref();

        assert!(matches!(time_zone_ref.unix_leap_time_to_unix_time(1136073621), Ok(1136073599)));
        assert!(matches!(time_zone_ref.unix_leap_time_to_unix_time(1136073622), Ok(1136073600)));
        assert!(matches!(time_zone_ref.unix_leap_time_to_unix_time(1136073623), Ok(1136073600)));
        assert!(matches!(time_zone_ref.unix_leap_time_to_unix_time(1136073624), Ok(1136073601)));

        assert!(matches!(time_zone_ref.unix_time_to_unix_leap_time(1136073599), Ok(1136073621)));
        assert!(matches!(time_zone_ref.unix_time_to_unix_leap_time(1136073600), Ok(1136073623)));
        assert!(matches!(time_zone_ref.unix_time_to_unix_leap_time(1136073601), Ok(1136073624)));

        Ok(())
    }

    #[test]
    fn test_leap_seconds_overflow() -> Result<(), Error> {
        let time_zone_err = TimeZone::new(
            vec![Transition::new(i64::MIN, 0)],
            vec![LocalTimeType::UTC],
            vec![LeapSecond::new(0, 1)],
            Some(TransitionRule::from(LocalTimeType::UTC)),
        );
        assert!(time_zone_err.is_err());

        let time_zone = TimeZone::new(
            vec![Transition::new(i64::MAX, 0)],
            vec![LocalTimeType::UTC],
            vec![LeapSecond::new(0, 1)],
            None,
        )?;
        assert!(matches!(
            time_zone.find_local_time_type(i64::MAX),
            Err(Error::FindLocalTimeType(_))
        ));

        Ok(())
    }
}
