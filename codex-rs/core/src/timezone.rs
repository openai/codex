//! Timezone handling utilities for consistent time display across the application.

use chrono::DateTime;
use chrono::Local;
use chrono::Utc;
use std::env;

/// Timezone preference for displaying timestamps
#[derive(Debug, Clone, PartialEq)]
pub enum TimezonePreference {
    /// Use UTC timezone
    Utc,
    /// Use local system timezone
    Local,
    /// Use a specific timezone offset (in seconds from UTC)
    Offset(i32),
}

impl Default for TimezonePreference {
    fn default() -> Self {
        // Check environment variable for default preference
        match env::var("CODEX_TIMEZONE").as_deref() {
            Ok("utc") | Ok("UTC") => TimezonePreference::Utc,
            Ok("local") | Ok("LOCAL") => TimezonePreference::Local,
            Ok(offset_str) => {
                if let Ok(offset) = offset_str.parse::<i32>() {
                    TimezonePreference::Offset(offset)
                } else {
                    TimezonePreference::Local
                }
            }
            _ => TimezonePreference::Utc,
        }
    }
}

impl TimezonePreference {
    /// Create a TimezonePreference from a config string
    pub fn from_config(config_value: &str) -> Self {
        match config_value.to_lowercase().as_str() {
            "local" => TimezonePreference::Local,
            "utc" => TimezonePreference::Utc,
            offset_str => {
                if let Ok(offset) = offset_str.parse::<i32>() {
                    TimezonePreference::Offset(offset)
                } else {
                    // Fallback to environment variable or default (UTC for backward compatibility)
                    Self::default()
                }
            }
        }
    }
}

/// Format a UTC timestamp according to the timezone preference
pub fn format_timestamp(utc_time: DateTime<Utc>, preference: &TimezonePreference) -> String {
    match preference {
        TimezonePreference::Utc => utc_time.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        TimezonePreference::Local => {
            let local_time = utc_time.with_timezone(&Local);
            local_time.format("%Y-%m-%d %H:%M:%S %Z").to_string()
        }
        TimezonePreference::Offset(offset_seconds) => {
            // Use a safe range for timezone offsets (-18 to +18 hours)
            let safe_offset = (*offset_seconds).clamp(-18 * 3600, 18 * 3600);
            if let Some(offset) = chrono::FixedOffset::east_opt(safe_offset) {
                let offset_time = utc_time.with_timezone(&offset);
                offset_time.format("%Y-%m-%d %H:%M:%S %z").to_string()
            } else {
                // Fallback to UTC if offset creation fails
                utc_time.format("%Y-%m-%d %H:%M:%S UTC").to_string()
            }
        }
    }
}

/// Generate a timestamp string for logging (always UTC with RFC3339 format)
pub fn now_ts() -> String {
    // RFC3339 for readability; consumers can parse as needed.
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Generate a timestamp string for display purposes
pub fn now_display(preference: &TimezonePreference) -> String {
    format_timestamp(Utc::now(), preference)
}

/// Convert a human-readable time difference to display format
pub fn human_time_ago_with_tz(ts: DateTime<Utc>, preference: &TimezonePreference) -> String {
    let now = Utc::now();
    let delta = now - ts;
    let secs = delta.num_seconds();

    let relative_time = if secs < 60 {
        let n = secs.max(0);
        if n == 1 {
            format!("{n} second ago")
        } else {
            format!("{n} seconds ago")
        }
    } else if secs < 60 * 60 {
        let m = secs / 60;
        if m == 1 {
            format!("{m} minute ago")
        } else {
            format!("{m} minutes ago")
        }
    } else if secs < 60 * 60 * 24 {
        let h = secs / 3600;
        if h == 1 {
            format!("{h} hour ago")
        } else {
            format!("{h} hours ago")
        }
    } else {
        let d = secs / (60 * 60 * 24);
        if d == 1 {
            format!("{d} day ago")
        } else {
            format!("{d} days ago")
        }
    };

    // For recent times, show relative time only
    if secs < 60 * 60 * 24 {
        relative_time
    } else {
        // For older times, show both relative and absolute time
        let absolute_time = format_timestamp(ts, preference);
        format!("{relative_time} ({absolute_time})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use chrono::Utc;

    #[test]
    fn test_timezone_preference_default() {
        // Test default behavior (should be UTC unless env var is set, for backward compatibility)
        let pref = TimezonePreference::default();
        // We can't assert the exact value since it depends on environment
        // but we can test that it doesn't panic
        assert!(matches!(
            pref,
            TimezonePreference::Local | TimezonePreference::Utc | TimezonePreference::Offset(_)
        ));
    }

    #[test]
    fn test_format_timestamp_utc() {
        let utc_time = Utc::now();
        let formatted = format_timestamp(utc_time, &TimezonePreference::Utc);
        assert!(formatted.contains("UTC"));
    }

    #[test]
    fn test_format_timestamp_local() {
        let utc_time = Utc::now();
        let formatted = format_timestamp(utc_time, &TimezonePreference::Local);
        // Should contain some timezone indicator
        assert!(formatted.len() > 19); // Basic timestamp is 19 chars
    }

    #[test]
    fn test_format_timestamp_offset() {
        let utc_time = Utc::now();
        let formatted = format_timestamp(utc_time, &TimezonePreference::Offset(3600)); // +1 hour
        assert!(formatted.contains("+01"));
    }

    #[test]
    fn test_human_time_ago() {
        let now = Utc::now();
        let five_minutes_ago = now - Duration::minutes(5);

        let result = human_time_ago_with_tz(five_minutes_ago, &TimezonePreference::Utc);
        assert!(result.contains("5 minutes ago"));
    }

    #[test]
    fn test_human_time_ago_old() {
        let now = Utc::now();
        let two_days_ago = now - Duration::days(2);

        let result = human_time_ago_with_tz(two_days_ago, &TimezonePreference::Utc);
        assert!(result.contains("2 days ago"));
        assert!(result.contains("UTC")); // Should include absolute time for old timestamps
    }
}
#[test]
fn test_from_config() {
    assert_eq!(
        TimezonePreference::from_config("utc"),
        TimezonePreference::Utc
    );
    assert_eq!(
        TimezonePreference::from_config("UTC"),
        TimezonePreference::Utc
    );
    assert_eq!(
        TimezonePreference::from_config("local"),
        TimezonePreference::Local
    );
    assert_eq!(
        TimezonePreference::from_config("LOCAL"),
        TimezonePreference::Local
    );
    assert_eq!(
        TimezonePreference::from_config("3600"),
        TimezonePreference::Offset(3600)
    );
    assert_eq!(
        TimezonePreference::from_config("-18000"),
        TimezonePreference::Offset(-18000)
    );

    // Invalid values should fall back to default
    let result = TimezonePreference::from_config("invalid");
    assert!(matches!(
        result,
        TimezonePreference::Local | TimezonePreference::Utc | TimezonePreference::Offset(_)
    ));
}
