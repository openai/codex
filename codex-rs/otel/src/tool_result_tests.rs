use super::*;
use pretty_assertions::assert_eq;

#[test]
fn telemetry_preview_returns_original_within_limits() {
    // A literal marker in the tool's own output must not imply telemetry truncation.
    for content in ["", "first\r\nsecond\r\n", TRUNCATION_NOTICE] {
        let preview = telemetry_preview(content, ToolResultLogConfig::default());
        assert!(matches!(preview.text, Cow::Borrowed(_)));
        assert_eq!(
            preview,
            ToolResultPreview {
                text: Cow::Borrowed(content),
                truncated: false
            }
        );
    }
}

#[test]
fn telemetry_preview_truncates_by_bytes() {
    let limits = ToolResultLogConfig::default();
    let content = "x".repeat(limits.max_bytes + 8);
    let preview = telemetry_preview(&content, limits);
    assert!(preview.truncated);
    assert!(preview.text.contains(TRUNCATION_NOTICE));
    assert!(preview.text.len() <= limits.max_bytes + TRUNCATION_NOTICE.len() + 1);
    assert_eq!(
        telemetry_preview(
            &content,
            ToolResultLogConfig {
                max_bytes: content.len(),
            }
        ),
        ToolResultPreview {
            text: Cow::Borrowed(&content),
            truncated: false
        }
    );
    assert_eq!(
        telemetry_preview("éé", ToolResultLogConfig { max_bytes: 3 }),
        ToolResultPreview {
            text: Cow::Owned(format!("é\n{TRUNCATION_NOTICE}")),
            truncated: true
        }
    );
    for (content, max_bytes, expected, truncated) in [
        ("", 0, "".to_string(), false),
        ("\nsecret", 0, TRUNCATION_NOTICE.to_string(), true),
        (
            "first\r\nsecond",
            7,
            format!("first\r\n{TRUNCATION_NOTICE}"),
            true,
        ),
        ("first\r\n", 7, "first\r\n".to_string(), false),
        ("\n\nsecret", 1, format!("\n{TRUNCATION_NOTICE}"), true),
    ] {
        assert_eq!(
            telemetry_preview(content, ToolResultLogConfig { max_bytes }),
            ToolResultPreview {
                text: Cow::Owned(expected),
                truncated
            }
        );
    }
}
