//! Integration tests for timezone functionality

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use chrono::Utc;
    use codex_core::timezone::TimezonePreference;
    use codex_core::timezone::format_timestamp;
    use codex_core::timezone::human_time_ago_with_tz;

    #[test]
    fn test_timezone_integration() {
        let now = Utc::now();
        let five_minutes_ago = now - Duration::minutes(5);

        // Test UTC preference
        let utc_pref = TimezonePreference::Utc;
        let utc_formatted = format_timestamp(now, &utc_pref);
        assert!(utc_formatted.contains("UTC"));

        let utc_relative = human_time_ago_with_tz(five_minutes_ago, &utc_pref);
        assert!(utc_relative.contains("5 minutes ago"));

        // Test local preference
        let local_pref = TimezonePreference::Local;
        let local_formatted = format_timestamp(now, &local_pref);
        assert!(local_formatted.len() > 19); // Should have timezone info

        // Test offset preference (UTC+1)
        let offset_pref = TimezonePreference::Offset(3600);
        let offset_formatted = format_timestamp(now, &offset_pref);
        assert!(offset_formatted.contains("+01"));
    }

    #[test]
    fn test_config_parsing() {
        // Test various config string formats
        assert_eq!(
            TimezonePreference::from_config("utc"),
            TimezonePreference::Utc
        );
        assert_eq!(
            TimezonePreference::from_config("local"),
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
    }

    #[test]
    fn test_old_timestamps_show_absolute_time() {
        let now = Utc::now();
        let two_days_ago = now - Duration::days(2);

        let pref = TimezonePreference::Utc;
        let result = human_time_ago_with_tz(two_days_ago, &pref);

        // Should contain both relative and absolute time for old timestamps
        assert!(result.contains("2 days ago"));
        assert!(result.contains("UTC"));
    }
}
