use chrono::Utc;

/// Returns a string representing the elapsed time since `start_time` like
/// "1m15s" or "1.50s".
pub fn format_elapsed(start_time: chrono::DateTime<Utc>) -> String {
    let elapsed = Utc::now().signed_duration_since(start_time);
    format_time_delta(elapsed)
}

fn format_time_delta(elapsed: chrono::TimeDelta) -> String {
    let millis = elapsed.num_milliseconds();
    format_elapsed_millis(millis)
}

pub fn format_duration(duration: std::time::Duration) -> String {
    let millis = duration.as_millis() as i64;
    format_elapsed_millis(millis)
}

fn format_elapsed_millis(millis: i64) -> String {
    if millis < 1000 {
        format!("{}ms", millis)
    } else if millis < 60_000 {
        format!("{:.2}s", millis as f64 / 1000.0)
    } else {
        let minutes = millis / 60_000;
        let seconds = (millis % 60_000) / 1000;
        format!("{minutes}m{seconds:02}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_format_time_delta_subsecond() {
        // Durations < 1s should be rendered in milliseconds with no decimals.
        let dur = Duration::milliseconds(250);
        assert_eq!(format_time_delta(dur), "250ms");

        // Exactly zero should still work.
        let dur_zero = Duration::milliseconds(0);
        assert_eq!(format_time_delta(dur_zero), "0ms");
    }

    #[test]
    fn test_format_time_delta_seconds() {
        // Durations between 1s (inclusive) and 60s (exclusive) should be
        // printed with 2-decimal-place seconds.
        let dur = Duration::milliseconds(1_500); // 1.5s
        assert_eq!(format_time_delta(dur), "1.50s");

        // 59.999s rounds to 60.00s
        let dur2 = Duration::milliseconds(59_999);
        assert_eq!(format_time_delta(dur2), "60.00s");
    }

    #[test]
    fn test_format_time_delta_minutes() {
        // Durations ≥ 1 minute should be printed mmss.
        let dur = Duration::milliseconds(75_000); // 1m15s
        assert_eq!(format_time_delta(dur), "1m15s");

        let dur_exact = Duration::milliseconds(60_000); // 1m0s
        assert_eq!(format_time_delta(dur_exact), "1m00s");

        let dur_long = Duration::milliseconds(3_601_000);
        assert_eq!(format_time_delta(dur_long), "60m01s");
    }
}
