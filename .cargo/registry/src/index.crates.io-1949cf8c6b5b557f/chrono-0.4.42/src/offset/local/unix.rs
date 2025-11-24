// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::{cell::RefCell, collections::hash_map, env, fs, hash::Hasher, time::SystemTime};

use super::tz_info::TimeZone;
use super::{FixedOffset, NaiveDateTime};
use crate::MappedLocalTime;

pub(super) fn offset_from_utc_datetime(utc: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
    offset(utc, false)
}

pub(super) fn offset_from_local_datetime(local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
    offset(local, true)
}

fn offset(d: &NaiveDateTime, local: bool) -> MappedLocalTime<FixedOffset> {
    TZ_INFO.with(|maybe_cache| {
        maybe_cache.borrow_mut().get_or_insert_with(Cache::default).offset(*d, local)
    })
}

// we have to store the `Cache` in an option as it can't
// be initialized in a static context.
thread_local! {
    static TZ_INFO: RefCell<Option<Cache>> = Default::default();
}

enum Source {
    LocalTime { mtime: SystemTime },
    Environment { hash: u64 },
}

impl Source {
    fn new(env_tz: Option<&str>) -> Source {
        match env_tz {
            Some(tz) => {
                let mut hasher = hash_map::DefaultHasher::new();
                hasher.write(tz.as_bytes());
                let hash = hasher.finish();
                Source::Environment { hash }
            }
            None => match fs::symlink_metadata("/etc/localtime") {
                Ok(data) => Source::LocalTime {
                    // we have to pick a sensible default when the mtime fails
                    // by picking SystemTime::now() we raise the probability of
                    // the cache being invalidated if/when the mtime starts working
                    mtime: data.modified().unwrap_or_else(|_| SystemTime::now()),
                },
                Err(_) => {
                    // as above, now() should be a better default than some constant
                    // TODO: see if we can improve caching in the case where the fallback is a valid timezone
                    Source::LocalTime { mtime: SystemTime::now() }
                }
            },
        }
    }
}

struct Cache {
    zone: TimeZone,
    source: Source,
    last_checked: SystemTime,
}

#[cfg(target_os = "aix")]
const TZDB_LOCATION: &str = "/usr/share/lib/zoneinfo";

#[cfg(not(any(target_os = "android", target_os = "aix", target_env = "ohos")))]
const TZDB_LOCATION: &str = "/usr/share/zoneinfo";

fn fallback_timezone() -> Option<TimeZone> {
    let tz_name = iana_time_zone::get_timezone().ok()?;
    #[cfg(not(any(target_os = "android", target_env = "ohos")))]
    let bytes = fs::read(format!("{TZDB_LOCATION}/{tz_name}")).ok()?;
    #[cfg(any(target_os = "android", target_env = "ohos"))]
    let bytes = crate::offset::local::tz_data::for_zone(&tz_name).ok()??;
    TimeZone::from_tz_data(&bytes).ok()
}

impl Default for Cache {
    fn default() -> Cache {
        // default to UTC if no local timezone can be found
        let env_tz = env::var("TZ").ok();
        let env_ref = env_tz.as_deref();
        Cache {
            last_checked: SystemTime::now(),
            source: Source::new(env_ref),
            zone: current_zone(env_ref),
        }
    }
}

fn current_zone(var: Option<&str>) -> TimeZone {
    TimeZone::local(var).ok().or_else(fallback_timezone).unwrap_or_else(TimeZone::utc)
}

impl Cache {
    fn offset(&mut self, d: NaiveDateTime, local: bool) -> MappedLocalTime<FixedOffset> {
        let now = SystemTime::now();

        match now.duration_since(self.last_checked) {
            // If the cache has been around for less than a second then we reuse it
            // unconditionally. This is a reasonable tradeoff because the timezone
            // generally won't be changing _that_ often, but if the time zone does
            // change, it will reflect sufficiently quickly from an application
            // user's perspective.
            Ok(d) if d.as_secs() < 1 => (),
            Ok(_) | Err(_) => {
                let env_tz = env::var("TZ").ok();
                let env_ref = env_tz.as_deref();
                let new_source = Source::new(env_ref);

                let out_of_date = match (&self.source, &new_source) {
                    // change from env to file or file to env, must recreate the zone
                    (Source::Environment { .. }, Source::LocalTime { .. })
                    | (Source::LocalTime { .. }, Source::Environment { .. }) => true,
                    // stay as file, but mtime has changed
                    (Source::LocalTime { mtime: old_mtime }, Source::LocalTime { mtime })
                        if old_mtime != mtime =>
                    {
                        true
                    }
                    // stay as env, but hash of variable has changed
                    (Source::Environment { hash: old_hash }, Source::Environment { hash })
                        if old_hash != hash =>
                    {
                        true
                    }
                    // cache can be reused
                    _ => false,
                };

                if out_of_date {
                    self.zone = current_zone(env_ref);
                }

                self.last_checked = now;
                self.source = new_source;
            }
        }

        if !local {
            let offset = self
                .zone
                .find_local_time_type(d.and_utc().timestamp())
                .expect("unable to select local time type")
                .offset();

            return match FixedOffset::east_opt(offset) {
                Some(offset) => MappedLocalTime::Single(offset),
                None => MappedLocalTime::None,
            };
        }

        // we pass through the year as the year of a local point in time must either be valid in that locale, or
        // the entire time was skipped in which case we will return MappedLocalTime::None anyway.
        self.zone
            .find_local_time_type_from_local(d)
            .expect("unable to select local time type")
            .and_then(|o| FixedOffset::east_opt(o.offset()))
    }
}
