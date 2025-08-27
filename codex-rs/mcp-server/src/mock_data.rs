use std::path::Path;
use std::path::PathBuf;

use codex_protocol::mcp_protocol::CodeLocation;
use codex_protocol::mcp_protocol::Finding;
use codex_protocol::mcp_protocol::LineRange;
use codex_protocol::mcp_protocol::ReviewOutput;
use codex_protocol::mcp_protocol::RunSubagentResponse;
use codex_protocol::mcp_protocol::Subagent;
use codex_protocol::mcp_protocol::SubagentOutput;

/// Build a mock response for Review subagent using actual unified diff text.
pub(crate) fn subagent_mock_response_from_diff(
    subagent: Subagent,
    cwd: &Path,
    diff: &str,
) -> RunSubagentResponse {
    match subagent {
        Subagent::Review => {
            let findings = review_findings_from_unified_diff(cwd, diff);
            RunSubagentResponse {
                output: SubagentOutput::Review(ReviewOutput { findings }),
            }
        }
    }
}

/// Parse a unified diff and generate representative findings mapped to changed hunks.
fn review_findings_from_unified_diff(cwd: &Path, diff: &str) -> Vec<Finding> {
    const TITLES: &[&str] = &[
        "Add a clarifying comment",
        "Consider extracting a helper function",
        "Prefer descriptive variable names",
        "Validate inputs and handle errors early",
        "Document the intent of this change",
        "Consider reducing nesting with early returns",
        "Add unit tests for this branch",
        "Ensure consistent logging and levels",
    ];
    const BODIES: &[&str] = &[
        "Add a comment to this line to explain the rationale.",
        "This logic could be extracted for readability and reuse.",
        "Use a more descriptive identifier to clarify the purpose.",
        "Add a guard clause to handle invalid or edge inputs.",
        "Add a doc comment describing the behavior for maintainers.",
        "Flatten control flow using early returns where safe.",
        "Add a focused test that covers this behavior.",
        "Use the shared logger and appropriate log level.",
    ];

    let mut findings: Vec<Finding> = Vec::new();
    let mut current_file: Option<PathBuf> = None;
    let mut in_hunk: bool = false;
    let mut new_line: u32 = 1;
    let mut template_index: usize = 0;

    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            current_file = None;
            in_hunk = false;
            continue;
        }

        if let Some(rest) = line.strip_prefix("+++ b/") {
            current_file = Some(cwd.join(rest.trim()));
            continue;
        }
        if line.starts_with("+++ ") || line.starts_with("--- ") {
            continue;
        }

        if let Some(hunk_header) = line.strip_prefix("@@") {
            if let Some((_, after_plus)) = hunk_header.split_once('+') {
                let mut range_text = after_plus.trim();
                if let Some((seg, _)) = range_text.split_once(' ') {
                    range_text = seg;
                }
                let (start, _count) = parse_start_count(range_text);
                new_line = start;
                in_hunk = true;
            }
            continue;
        }

        if in_hunk {
            if line.starts_with(' ') {
                new_line = new_line.saturating_add(1);
            } else if line.starts_with('-') {
                // deletion: no advance of new_line
            } else if line.starts_with('+') && !line.starts_with("+++") {
                if let Some(path) = &current_file {
                    let title = TITLES[template_index % TITLES.len()].to_string();
                    let mut body = BODIES[template_index % BODIES.len()].to_string();
                    let snippet = line.trim_start_matches('+').trim();
                    if !snippet.is_empty() {
                        body.push_str("\nSnippet: ");
                        let truncated = if snippet.len() > 140 {
                            let mut s = snippet[..140].to_string();
                            s.push('…');
                            s
                        } else {
                            snippet.to_string()
                        };
                        body.push_str(&truncated);
                    }

                    findings.push(Finding {
                        title,
                        body,
                        confidence_score: confidence_for_index(template_index),
                        code_location: CodeLocation {
                            absolute_file_path: to_forward_slashes(path),
                            line_range: LineRange {
                                start: new_line,
                                end: new_line,
                            },
                        },
                    });
                    template_index += 1;
                }
                new_line = new_line.saturating_add(1);
            }
        }
    }

    if findings.len() > 50 {
        findings.truncate(50);
    }
    findings
}

fn confidence_for_index(i: usize) -> f32 {
    let base = 0.72f32;
    let step = (i as f32 % 7.0) * 0.03;
    (base + step).min(0.95)
}

fn parse_start_count(text: &str) -> (u32, u32) {
    // Formats: "123,45" or just "123"
    if let Some((start_str, count_str)) = text.split_once(',') {
        let start = start_str
            .trim()
            .trim_start_matches('+')
            .parse()
            .unwrap_or(1);
        let count = count_str.trim().parse().unwrap_or(1);
        (start as u32, count as u32)
    } else {
        let start = text.trim().trim_start_matches('+').parse().unwrap_or(1);
        (start as u32, 1)
    }
}

fn to_forward_slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn returns_empty_findings_for_empty_diff() {
        let cwd = Path::new("/tmp");
        let resp = subagent_mock_response_from_diff(Subagent::Review, cwd, "");
        match resp.output {
            SubagentOutput::Review(ReviewOutput { findings }) => {
                assert!(findings.is_empty(), "Expected no findings for empty diff");
            }
        }
    }

    #[test]
    fn generates_findings_for_added_lines_with_correct_locations() {
        let cwd = Path::new("/repo");
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -10,2 +10,4 @@
 context
+let x = 1;
+// TODO: add docs
 context
@@ -50,3 +52,5 @@
-context
+context changed
+fn new_fn() {}
+// comment
"#;

        let resp = subagent_mock_response_from_diff(Subagent::Review, cwd, diff);
        match resp.output {
            SubagentOutput::Review(ReviewOutput { findings }) => {
                // Added lines: 2 in first hunk, 3 in second hunk => 5 findings
                assert_eq!(findings.len(), 5, "Expected one finding per added line");

                // Validate file path and line numbers for the first two additions
                let file_path = "/repo/src/lib.rs".to_string();
                assert_eq!(findings[0].code_location.absolute_file_path, file_path);
                assert_eq!(findings[0].code_location.line_range.start, 11);
                assert_eq!(findings[0].code_location.line_range.end, 11);
                assert_eq!(findings[1].code_location.absolute_file_path, file_path);
                assert_eq!(findings[1].code_location.line_range.start, 12);
                assert_eq!(findings[1].code_location.line_range.end, 12);

                // Validate second hunk first two additions start at 52, then 53
                assert_eq!(findings[2].code_location.line_range.start, 52);
                assert_eq!(findings[3].code_location.line_range.start, 53);
            }
        }
    }
}
