use serde::Deserialize;
use serde::Serialize;

/// Condition that controls loop execution.
///
/// # Examples
///
/// ```rust,ignore
/// use codex_core::loop_driver::LoopCondition;
///
/// // Parse from string
/// let iters = LoopCondition::parse("5").unwrap();
/// let duration = LoopCondition::parse("1h").unwrap();
///
/// // Create directly
/// let iters = LoopCondition::Iters { count: 5 };
/// let duration = LoopCondition::Duration { seconds: 3600 };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoopCondition {
    /// Run a fixed number of iterations.
    Iters {
        /// Number of iterations to run.
        count: i32,
    },
    /// Run until duration elapsed.
    Duration {
        /// Duration in seconds.
        seconds: i64,
    },
}

impl LoopCondition {
    /// Parse from CLI string.
    ///
    /// Accepts:
    /// - Iters: "5", "10", "100"
    /// - Duration: "5s", "10m", "2h", "1d"
    ///
    /// # Errors
    ///
    /// Returns error if string doesn't match expected format.
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();

        // Try duration first (has unit suffix)
        if let Some(seconds) = Self::try_parse_duration(s) {
            return Ok(Self::Duration { seconds });
        }

        // Fall back to iterations (plain number)
        s.parse::<i32>()
            .map(|count| Self::Iters { count })
            .map_err(|_| {
                format!(
                    "Invalid loop condition: '{s}'. Expected iterations (e.g., '5') or duration (e.g., '1h', '30m', '5s')"
                )
            })
    }

    fn try_parse_duration(s: &str) -> Option<i64> {
        if s.len() < 2 {
            return None;
        }

        let (num_str, unit) = s.split_at(s.len() - 1);
        let value: i64 = num_str.parse().ok()?;

        let multiplier = match unit {
            "s" => 1,
            "m" => 60,
            "h" => 3600,
            "d" => 86400,
            _ => return None,
        };

        Some(value * multiplier)
    }

    /// Get display string for this condition.
    pub fn display(&self) -> String {
        match self {
            Self::Iters { count } => format!("{count} iterations"),
            Self::Duration { seconds } => {
                if *seconds >= 86400 {
                    format!("{}d", seconds / 86400)
                } else if *seconds >= 3600 {
                    format!("{}h", seconds / 3600)
                } else if *seconds >= 60 {
                    format!("{}m", seconds / 60)
                } else {
                    format!("{seconds}s")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_iters() {
        assert_eq!(
            LoopCondition::parse("5"),
            Ok(LoopCondition::Iters { count: 5 })
        );
        assert_eq!(
            LoopCondition::parse("100"),
            Ok(LoopCondition::Iters { count: 100 })
        );
    }

    #[test]
    fn parse_duration() {
        assert_eq!(
            LoopCondition::parse("5s"),
            Ok(LoopCondition::Duration { seconds: 5 })
        );
        assert_eq!(
            LoopCondition::parse("10m"),
            Ok(LoopCondition::Duration { seconds: 600 })
        );
        assert_eq!(
            LoopCondition::parse("2h"),
            Ok(LoopCondition::Duration { seconds: 7200 })
        );
        assert_eq!(
            LoopCondition::parse("1d"),
            Ok(LoopCondition::Duration { seconds: 86400 })
        );
    }

    #[test]
    fn parse_invalid() {
        assert!(LoopCondition::parse("abc").is_err());
        assert!(LoopCondition::parse("5x").is_err());
        assert!(LoopCondition::parse("").is_err());
    }

    #[test]
    fn display() {
        assert_eq!(LoopCondition::Iters { count: 5 }.display(), "5 iterations");
        assert_eq!(LoopCondition::Duration { seconds: 3600 }.display(), "1h");
    }
}
