pub(crate) fn format_goal_elapsed_seconds(seconds: i64) -> String {
    let seconds = seconds.max(0) as u64;
    if seconds < 60 {
        return format!("{seconds}s");
    }

    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }

    let hours = minutes / 60;
    let remaining_minutes = minutes % 60;
    if remaining_minutes == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h {remaining_minutes}m")
    }
}

#[cfg(test)]
mod tests {
    use super::format_goal_elapsed_seconds;
    use pretty_assertions::assert_eq;

    #[test]
    fn format_goal_elapsed_seconds_is_compact() {
        assert_eq!(format_goal_elapsed_seconds(/*seconds*/ 0), "0s");
        assert_eq!(format_goal_elapsed_seconds(/*seconds*/ 59), "59s");
        assert_eq!(format_goal_elapsed_seconds(/*seconds*/ 60), "1m");
        assert_eq!(format_goal_elapsed_seconds(30 * 60), "30m");
        assert_eq!(format_goal_elapsed_seconds(90 * 60), "1h 30m");
        assert_eq!(format_goal_elapsed_seconds(2 * 60 * 60), "2h");
    }
}
