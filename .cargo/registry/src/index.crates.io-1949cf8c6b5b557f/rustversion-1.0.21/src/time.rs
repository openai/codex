use crate::date::Date;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

// Timestamp of 2016-03-01 00:00:00 in UTC.
const BASE: u64 = 1456790400;
const BASE_YEAR: u16 = 2016;
const BASE_MONTH: u8 = 3;

// Days between leap days.
const CYCLE: u64 = 365 * 4 + 1;

const DAYS_BY_MONTH: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

pub fn today() -> Date {
    let default = Date {
        year: 2025,
        month: 2,
        day: 25,
    };
    try_today().unwrap_or(default)
}

fn try_today() -> Option<Date> {
    if let Some(pkg_name) = env::var_os("CARGO_PKG_NAME") {
        if pkg_name.to_str() == Some("rustversion-tests") {
            return None; // Stable date for ui testing.
        }
    }

    let now = SystemTime::now();
    let since_epoch = now.duration_since(UNIX_EPOCH).ok()?;
    let secs = since_epoch.as_secs();

    let approx_days = secs.checked_sub(BASE)? / 60 / 60 / 24;
    let cycle = approx_days / CYCLE;
    let mut rem = approx_days % CYCLE;

    let mut year = BASE_YEAR + cycle as u16 * 4;
    let mut month = BASE_MONTH;
    loop {
        let days_in_month = DAYS_BY_MONTH[month as usize - 1];
        if rem < days_in_month as u64 {
            let day = rem as u8 + 1;
            return Some(Date { year, month, day });
        }
        rem -= days_in_month as u64;
        year += (month == 12) as u16;
        month = month % 12 + 1;
    }
}
