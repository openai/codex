//! Preference precedence and native pattern parsing without changing host settings.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn time_locale_uses_first_nonempty_setting() {
    for (values, expected) in [
        (
            [Some("C"), Some("en_US.UTF-8"), Some("de_DE.UTF-8")],
            Some("C"),
        ),
        (
            [Some(""), Some("C.UTF-8"), Some("en_US.UTF-8")],
            Some("C.UTF-8"),
        ),
        ([None, Some(""), Some("C.utf8")], Some("C.utf8")),
        ([Some("invalid"), Some("C"), None], Some("invalid")),
        ([None, Some(""), None], None),
    ] {
        let environment = ["LC_ALL", "LC_TIME", "LANG"].into_iter().zip(values);
        assert_eq!(
            time_locale(|name| environment
                .clone()
                .find(|(key, _)| *key == name)
                .unwrap()
                .1
                .map(Into::into)),
            expected.map(std::ffi::OsString::from),
        );
    }
}

#[test]
fn standard_locales_and_unavailable_locales() {
    for locale in ["C", "POSIX", "C.UTF-8", "C.utf8"] {
        assert_eq!(
            detect_time_locale(locale),
            Some(ClockFormat::TwentyFourHour)
        );
    }
    for locale in [
        "codex_missing_locale",
        "C.codex_missing_encoding",
        "invalid\0locale",
    ] {
        assert_eq!(detect_time_locale(locale), None);
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    assert_eq!(detect_time_locale("de_DE.UTF-8"), None);
}

#[test]
fn strftime_hour_fields_determine_the_clock() {
    for (pattern, expected) in [
        ("%I:%M:%S %p", Some(ClockFormat::TwelveHour)),
        ("%l:%M %P", Some(ClockFormat::TwelveHour)),
        ("%r", Some(ClockFormat::TwelveHour)),
        ("%H:%M:%S", Some(ClockFormat::TwentyFourHour)),
        ("%k:%M", Some(ClockFormat::TwentyFourHour)),
        ("%R", Some(ClockFormat::TwentyFourHour)),
        ("%T %Z", Some(ClockFormat::TwentyFourHour)),
        ("%%I %_2OH:%M", Some(ClockFormat::TwentyFourHour)),
        ("%%H %-I:%M", Some(ClockFormat::TwelveHour)),
        ("%H %I", None),
        ("%H %X", None),
        ("%H %", None),
        ("%H %Q", None),
        ("%p", None),
        ("%%H", None),
        ("", None),
    ] {
        assert_eq!(parse_strftime(pattern.as_bytes()), expected, "{pattern}");
    }
}

#[test]
fn native_patterns_ignore_literals_and_use_the_first_alternative() {
    for (pattern, symbols, expected) in [
        ("h:mm tt", "hH", Some(ClockFormat::TwelveHour)),
        ("HH:mm", "hH", Some(ClockFormat::TwentyFourHour)),
        ("h 'H; o''clock' a", "hHKk", Some(ClockFormat::TwelveHour)),
        ("'h' HH:mm;h:mm tt", "hH", Some(ClockFormat::TwentyFourHour)),
        ("h:mm tt;HH:mm", "hH", Some(ClockFormat::TwelveHour)),
        ("K a", "hHKk", Some(ClockFormat::TwelveHour)),
        ("kk", "hHKk", Some(ClockFormat::TwentyFourHour)),
        ("K HH:mm", "hH", Some(ClockFormat::TwentyFourHour)),
        ("h 'unterminated", "hH", None),
        ("h HH", "hH", None),
        ("'HH' mm", "hH", None),
        ("", "hH", None),
    ] {
        assert_eq!(
            parse_native_pattern(pattern, symbols),
            expected,
            "{pattern}"
        );
    }
}
