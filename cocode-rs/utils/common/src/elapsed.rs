use std::time::Duration;
use std::time::Instant;

/// Returns a string representing the elapsed time since `start_time` like
/// "1m 15s" or "1.50s".
pub fn format_elapsed(start_time: Instant) -> String {
    format_duration(start_time.elapsed())
}

/// Convert a [`std::time::Duration`] into a human-readable, compact string.
///
/// Formatting rules:
/// * < 1 s  ->  "{milli}ms"
/// * < 60 s ->  "{sec:.2}s" (two decimal places)
/// * >= 60 s ->  "{min}m {sec:02}s"
pub fn format_duration(duration: Duration) -> String {
    let millis = duration.as_millis() as i64;
    format_elapsed_millis(millis)
}

fn format_elapsed_millis(millis: i64) -> String {
    if millis < 1000 {
        format!("{millis}ms")
    } else if millis < 60_000 {
        format!("{:.2}s", millis as f64 / 1000.0)
    } else {
        let minutes = millis / 60_000;
        let seconds = (millis % 60_000) / 1000;
        format!("{minutes}m {seconds:02}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_subsecond() {
        // Durations < 1s should be rendered in milliseconds with no decimals.
        let dur = Duration::from_millis(250);
        assert_eq!(format_duration(dur), "250ms");

        // Exactly zero should still work.
        let dur_zero = Duration::from_millis(0);
        assert_eq!(format_duration(dur_zero), "0ms");
    }

    #[test]
    fn test_format_duration_seconds() {
        // Durations between 1s (inclusive) and 60s (exclusive) should be
        // printed with 2-decimal-place seconds.
        let dur = Duration::from_millis(1_500); // 1.5s
        assert_eq!(format_duration(dur), "1.50s");

        // 59.999s rounds to 60.00s
        let dur2 = Duration::from_millis(59_999);
        assert_eq!(format_duration(dur2), "60.00s");
    }

    #[test]
    fn test_format_duration_minutes() {
        // Durations ≥ 1 minute should be printed mmss.
        let dur = Duration::from_millis(75_000); // 1m15s
        assert_eq!(format_duration(dur), "1m 15s");

        let dur_exact = Duration::from_millis(60_000); // 1m0s
        assert_eq!(format_duration(dur_exact), "1m 00s");

        let dur_long = Duration::from_millis(3_601_000);
        assert_eq!(format_duration(dur_long), "60m 01s");
    }

    #[test]
    fn test_format_duration_one_hour_has_space() {
        let dur_hour = Duration::from_millis(3_600_000);
        assert_eq!(format_duration(dur_hour), "60m 00s");
    }
}
