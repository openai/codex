//! Code quality metrics for filtering low-quality files.
//!
//! Based on Tabby's implementation: crates/tabby-index/src/code/intelligence.rs

/// Code quality metrics for a source file.
///
/// Used to filter out binary files, generated code, logs, and other
/// files that shouldn't be indexed.
#[derive(Debug, Clone)]
pub struct CodeMetrics {
    /// Maximum line length in characters
    pub max_line_length: i32,
    /// Average line length in characters
    pub avg_line_length: f32,
    /// Fraction of alphanumeric characters (0.0 - 1.0)
    pub alphanum_fraction: f32,
    /// Total number of lines
    pub num_lines: i32,
    /// Fraction of numeric digits (0.0 - 1.0)
    pub number_fraction: f32,
}

impl CodeMetrics {
    /// Compute metrics from file content.
    ///
    /// Single-pass algorithm for efficiency.
    pub fn compute(content: &str) -> Self {
        let lines: Vec<&str> = content.lines().collect();
        let num_lines = lines.len() as i32;

        if num_lines == 0 {
            return Self {
                max_line_length: 0,
                avg_line_length: 0.0,
                alphanum_fraction: 0.0,
                num_lines: 0,
                number_fraction: 0.0,
            };
        }

        let max_line_length = lines.iter().map(|l| l.len() as i32).max().unwrap_or(0);
        let avg_line_length = content.len() as f32 / num_lines as f32;

        let total_chars = content.len() as f32;
        let mut alphanum_count = 0;
        let mut number_count = 0;

        for c in content.chars() {
            if c.is_alphanumeric() {
                alphanum_count += 1;
            }
            if c.is_ascii_digit() {
                number_count += 1;
            }
        }

        Self {
            max_line_length,
            avg_line_length,
            alphanum_fraction: if total_chars > 0.0 {
                alphanum_count as f32 / total_chars
            } else {
                0.0
            },
            num_lines,
            number_fraction: if total_chars > 0.0 {
                number_count as f32 / total_chars
            } else {
                0.0
            },
        }
    }
}

/// Thresholds for code quality filtering.
///
/// Based on Tabby's thresholds: crates/tabby-index/src/code/index.rs
pub mod thresholds {
    /// Maximum line length (filter minified/obfuscated code)
    pub const MAX_LINE_LENGTH: i32 = 300;
    /// Maximum average line length (filter single-line files)
    pub const AVG_LINE_LENGTH: f32 = 150.0;
    /// Minimum alphanumeric fraction (filter binary/non-text)
    pub const MIN_ALPHANUM_FRACTION: f32 = 0.25;
    /// Maximum number of lines (filter huge files)
    pub const MAX_NUM_LINES: i32 = 100_000;
    /// Maximum number fraction (filter data/log files)
    pub const MAX_NUMBER_FRACTION: f32 = 0.50;
}

/// Check if a file is suitable for indexing.
///
/// Returns `true` if the file passes all quality checks.
pub fn is_valid_file(content: &str) -> bool {
    let metrics = CodeMetrics::compute(content);

    metrics.num_lines > 0
        && metrics.max_line_length <= thresholds::MAX_LINE_LENGTH
        && metrics.avg_line_length <= thresholds::AVG_LINE_LENGTH
        && metrics.alphanum_fraction >= thresholds::MIN_ALPHANUM_FRACTION
        && metrics.num_lines <= thresholds::MAX_NUM_LINES
        && metrics.number_fraction <= thresholds::MAX_NUMBER_FRACTION
}

/// Check if a file is suitable for indexing, with detailed reason.
///
/// Returns `Ok(())` if valid, or `Err(reason)` if not.
pub fn validate_file(content: &str) -> std::result::Result<(), &'static str> {
    let metrics = CodeMetrics::compute(content);

    if metrics.num_lines == 0 {
        return Err("empty file");
    }
    if metrics.max_line_length > thresholds::MAX_LINE_LENGTH {
        return Err("line too long (>300 chars, likely minified)");
    }
    if metrics.avg_line_length > thresholds::AVG_LINE_LENGTH {
        return Err("avg line length too high (>150, likely single-line)");
    }
    if metrics.alphanum_fraction < thresholds::MIN_ALPHANUM_FRACTION {
        return Err("likely binary (alphanum < 25%)");
    }
    if metrics.num_lines > thresholds::MAX_NUM_LINES {
        return Err("file too large (>100k lines)");
    }
    if metrics.number_fraction > thresholds::MAX_NUMBER_FRACTION {
        return Err("likely data file (numbers > 50%)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_rust_code() {
        let code = r#"
fn main() {
    println!("Hello, world!");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
        assert!(is_valid_file(code));
    }

    #[test]
    fn test_empty_file() {
        assert!(!is_valid_file(""));
    }

    #[test]
    fn test_minified_code() {
        // Long single line (>300 chars)
        let minified = "a".repeat(500);
        assert!(!is_valid_file(&minified));
    }

    #[test]
    fn test_binary_content() {
        // Low alphanumeric fraction
        let binary = "\x00\x01\x02\x03\x04\x05".repeat(100);
        assert!(!is_valid_file(&binary));
    }

    #[test]
    fn test_data_file() {
        // High number fraction (>50% digits)
        // "123456789\n" has 9 digits / 10 chars = 90%
        let data = "123456789\n".repeat(100);
        assert!(!is_valid_file(&data));
    }

    #[test]
    fn test_validate_file_reasons() {
        assert_eq!(validate_file(""), Err("empty file"));
        assert_eq!(
            validate_file(&"a".repeat(500)),
            Err("line too long (>300 chars, likely minified)")
        );
    }
}
