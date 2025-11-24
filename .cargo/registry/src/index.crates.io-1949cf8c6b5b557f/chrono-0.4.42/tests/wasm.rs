//! Run this test with:
//! `env TZ="$(date +%z)" NOW="$(date +%s)" wasm-pack test --node -- --features wasmbind`
//!
//! The `TZ` and `NOW` variables are used to compare the results inside the WASM environment with
//! the host system.
//! The check will fail if the local timezone does not match one of the timezones defined below.

#![cfg(all(
    target_arch = "wasm32",
    feature = "wasmbind",
    feature = "clock",
    not(any(target_os = "emscripten", target_os = "wasi"))
))]

use chrono::prelude::*;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn now() {
    let utc: DateTime<Utc> = Utc::now();
    let local: DateTime<Local> = Local::now();

    // Ensure time set by the test script is correct
    let now = env!("NOW");
    let actual = NaiveDateTime::parse_from_str(&now, "%s").unwrap().and_utc();
    let diff = utc - actual;
    assert!(
        diff < chrono::TimeDelta::try_minutes(5).unwrap(),
        "expected {} - {} == {} < 5m (env var: {})",
        utc,
        actual,
        diff,
        now,
    );

    let tz = env!("TZ");
    eprintln!("testing with tz={}", tz);

    // Ensure offset retrieved when getting local time is correct
    let expected_offset = match tz {
        "ACST-9:30" => FixedOffset::east_opt(19 * 30 * 60).unwrap(),
        "Asia/Katmandu" => FixedOffset::east_opt(23 * 15 * 60).unwrap(), // No DST thankfully
        "EDT" | "EST4" | "-0400" => FixedOffset::east_opt(-4 * 60 * 60).unwrap(),
        "EST" | "-0500" => FixedOffset::east_opt(-5 * 60 * 60).unwrap(),
        "UTC0" | "+0000" => FixedOffset::east_opt(0).unwrap(),
        tz => panic!("unexpected TZ {}", tz),
    };
    assert_eq!(
        &expected_offset,
        local.offset(),
        "expected: {:?} local: {:?}",
        expected_offset,
        local.offset(),
    );
}

#[wasm_bindgen_test]
fn from_is_exact() {
    let now = js_sys::Date::new_0();

    let dt = DateTime::<Utc>::from(now.clone());

    assert_eq!(now.get_time() as i64, dt.timestamp_millis());
}

#[wasm_bindgen_test]
fn local_from_local_datetime() {
    let now = Local::now();
    let ndt = now.naive_local();
    let res = match Local.from_local_datetime(&ndt).single() {
        Some(v) => v,
        None => panic! {"Required for test!"},
    };
    assert_eq!(now, res);
}

#[wasm_bindgen_test]
fn convert_all_parts_with_milliseconds() {
    let time: DateTime<Utc> = "2020-12-01T03:01:55.974Z".parse().unwrap();
    let js_date = js_sys::Date::from(time);

    assert_eq!(js_date.get_utc_full_year(), 2020);
    assert_eq!(js_date.get_utc_month(), 11); // months are numbered 0..=11
    assert_eq!(js_date.get_utc_date(), 1);
    assert_eq!(js_date.get_utc_hours(), 3);
    assert_eq!(js_date.get_utc_minutes(), 1);
    assert_eq!(js_date.get_utc_seconds(), 55);
    assert_eq!(js_date.get_utc_milliseconds(), 974);
}
