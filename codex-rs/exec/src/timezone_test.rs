//! Test timezone functionality in exec module

#[cfg(test)]
mod tests {
    use super::super::event_processor_with_human_output::EventProcessorWithHumanOutput;
    use codex_core::config::Config;
    use std::path::PathBuf;

    #[test]
    fn test_event_processor_with_timezone_config() {
        // Create a minimal config with timezone preference
        let mut config = Config::load_with_cli_overrides(vec![], Default::default()).unwrap();
        config.timezone_preference = "utc".to_string();

        // Create event processor with timezone config
        let processor = EventProcessorWithHumanOutput::create_with_ansi(
            false, // no ANSI for test
            &config, None,
        );

        // Verify timezone preference is stored
        assert_eq!(processor.timezone_preference(), "utc");
    }

    #[test]
    fn test_event_processor_with_local_timezone() {
        let mut config = Config::load_with_cli_overrides(vec![], Default::default()).unwrap();
        config.timezone_preference = "local".to_string();

        let processor = EventProcessorWithHumanOutput::create_with_ansi(
            true, // with ANSI
            &config,
            Some(PathBuf::from("/tmp/test")),
        );

        assert_eq!(processor.timezone_preference(), "local");
    }

    #[test]
    fn test_timezone_override_from_cli() {
        use codex_core::config::ConfigOverrides;

        // Test that CLI timezone override works
        let overrides = ConfigOverrides {
            timezone_preference: Some("local".to_string()),
            ..Default::default()
        };

        let config = Config::load_from_base_config_with_overrides(
            Default::default(),
            overrides,
            std::env::temp_dir(),
        )
        .unwrap();

        assert_eq!(config.timezone_preference, "local");
    }

    #[test]
    fn test_timezone_formatting() {
        // Test that our timezone preference affects timestamp formatting
        use chrono::Utc;
        use codex_core::timezone::TimezonePreference;
        use codex_core::timezone::format_timestamp;

        let now = Utc::now();

        // Test UTC format
        let utc_pref = TimezonePreference::from_config("utc");
        let utc_formatted = format_timestamp(now, &utc_pref);
        assert!(utc_formatted.contains("UTC"));

        // Test local format (this will vary by system, so just check it doesn't panic)
        let local_pref = TimezonePreference::from_config("local");
        let local_formatted = format_timestamp(now, &local_pref);
        assert!(!local_formatted.is_empty());

        // Test custom offset
        let offset_pref = TimezonePreference::from_config("28800"); // UTC+8
        let offset_formatted = format_timestamp(now, &offset_pref);
        assert!(offset_formatted.contains("+08"));
    }
}
