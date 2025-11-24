#![cfg(all(unix, feature = "clock", feature = "std"))]

use std::{path, process, thread};

#[cfg(target_os = "linux")]
use chrono::Days;
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike};

fn verify_against_date_command_local(path: &'static str, dt: NaiveDateTime) {
    let output = process::Command::new(path)
        .arg("-d")
        .arg(format!("{}-{:02}-{:02} {:02}:05:01", dt.year(), dt.month(), dt.day(), dt.hour()))
        .arg("+%Y-%m-%d %H:%M:%S %:z")
        .output()
        .unwrap();

    let date_command_str = String::from_utf8(output.stdout).unwrap();

    // The below would be preferred. At this stage neither earliest() or latest()
    // seems to be consistent with the output of the `date` command, so we simply
    // compare both.
    // let local = Local
    //     .with_ymd_and_hms(year, month, day, hour, 5, 1)
    //     // looks like the "date" command always returns a given time when it is ambiguous
    //     .earliest();

    // if let Some(local) = local {
    //     assert_eq!(format!("{}\n", local), date_command_str);
    // } else {
    //     // we are in a "Spring forward gap" due to DST, and so date also returns ""
    //     assert_eq!("", date_command_str);
    // }

    // This is used while a decision is made whether the `date` output needs to
    // be exactly matched, or whether MappedLocalTime::Ambiguous should be handled
    // differently

    let date = NaiveDate::from_ymd_opt(dt.year(), dt.month(), dt.day()).unwrap();
    match Local.from_local_datetime(&date.and_hms_opt(dt.hour(), 5, 1).unwrap()) {
        chrono::MappedLocalTime::Ambiguous(a, b) => {
            assert!(format!("{a}\n") == date_command_str || format!("{b}\n") == date_command_str)
        }
        chrono::MappedLocalTime::Single(a) => {
            assert_eq!(format!("{a}\n"), date_command_str);
        }
        chrono::MappedLocalTime::None => {
            assert_eq!("", date_command_str);
        }
    }
}

/// path to Unix `date` command. Should work on most Linux and Unixes. Not the
/// path for MacOS (/bin/date) which uses a different version of `date` with
/// different arguments (so it won't run which is okay).
/// for testing only
#[allow(dead_code)]
#[cfg(not(target_os = "aix"))]
const DATE_PATH: &str = "/usr/bin/date";
#[allow(dead_code)]
#[cfg(target_os = "aix")]
const DATE_PATH: &str = "/opt/freeware/bin/date";

#[cfg(test)]
/// test helper to sanity check the date command behaves as expected
/// asserts the command succeeded
fn assert_run_date_version() {
    // note environment variable `LANG`
    match std::env::var_os("LANG") {
        Some(lang) => eprintln!("LANG: {lang:?}"),
        None => eprintln!("LANG not set"),
    }
    let out = process::Command::new(DATE_PATH).arg("--version").output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    // note the `date` binary version
    eprintln!("command: {DATE_PATH:?} --version\nstdout: {stdout:?}\nstderr: {stderr:?}");
    assert!(out.status.success(), "command failed: {DATE_PATH:?} --version");
}

#[test]
fn try_verify_against_date_command() {
    if !path::Path::new(DATE_PATH).exists() {
        eprintln!("date command {DATE_PATH:?} not found, skipping");
        return;
    }
    assert_run_date_version();

    eprintln!("Run command {DATE_PATH:?} for every hour from 1975 to 2077, skipping some years...",);

    let mut children = vec![];
    for year in [1975, 1976, 1977, 2020, 2021, 2022, 2073, 2074, 2075, 2076, 2077].iter() {
        children.push(thread::spawn(|| {
            let mut date = NaiveDate::from_ymd_opt(*year, 1, 1).unwrap().and_time(NaiveTime::MIN);
            let end = NaiveDate::from_ymd_opt(*year + 1, 1, 1).unwrap().and_time(NaiveTime::MIN);
            while date <= end {
                verify_against_date_command_local(DATE_PATH, date);
                date += chrono::TimeDelta::try_hours(1).unwrap();
            }
        }));
    }
    for child in children {
        // Wait for the thread to finish. Returns a result.
        let _ = child.join();
    }
}

#[cfg(target_os = "linux")]
fn verify_against_date_command_format_local(path: &'static str, dt: NaiveDateTime) {
    let required_format =
        "d%d D%D F%F H%H I%I j%j k%k l%l m%m M%M q%q S%S T%T u%u U%U w%w W%W X%X y%y Y%Y z%:z";
    // a%a - depends from localization
    // A%A - depends from localization
    // b%b - depends from localization
    // B%B - depends from localization
    // h%h - depends from localization
    // c%c - depends from localization
    // p%p - depends from localization
    // r%r - depends from localization
    // x%x - fails, date is dd/mm/yyyy, chrono is dd/mm/yy, same as %D
    // Z%Z - too many ways to represent it, will most likely fail

    let output = process::Command::new(path)
        .env("LANG", "c")
        .env("LC_ALL", "c")
        .arg("-d")
        .arg(format!(
            "{}-{:02}-{:02} {:02}:{:02}:{:02}",
            dt.year(),
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second()
        ))
        .arg(format!("+{required_format}"))
        .output()
        .unwrap();

    let date_command_str = String::from_utf8(output.stdout).unwrap();
    let date = NaiveDate::from_ymd_opt(dt.year(), dt.month(), dt.day()).unwrap();
    let ldt = Local
        .from_local_datetime(&date.and_hms_opt(dt.hour(), dt.minute(), dt.second()).unwrap())
        .unwrap();
    let formatted_date = format!("{}\n", ldt.format(required_format));
    assert_eq!(date_command_str, formatted_date);
}

#[test]
#[cfg(target_os = "linux")]
fn try_verify_against_date_command_format() {
    if !path::Path::new(DATE_PATH).exists() {
        eprintln!("date command {DATE_PATH:?} not found, skipping");
        return;
    }
    assert_run_date_version();

    let mut date = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap().and_hms_opt(12, 11, 13).unwrap();
    while date.year() < 2008 {
        verify_against_date_command_format_local(DATE_PATH, date);
        date = date + Days::new(55);
    }
}
