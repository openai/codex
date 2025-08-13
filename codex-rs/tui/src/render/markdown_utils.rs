/// Returns true if the provided text contains an unclosed fenced code block
/// (opened by ``` or ~~~, closed by a matching fence on its own line).
pub fn is_inside_unclosed_fence(s: &str) -> bool {
    let mut open = false;
    for line in s.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            if !open {
                open = true;
            } else {
                // closing fence on same pattern toggles off
                open = false;
            }
        }
    }
    open
}

/// Remove fenced code blocks that contain no content (whitespace-only) to avoid
/// streaming empty code blocks like ```lang\n``` or ```\n```.
pub fn strip_empty_fenced_code_blocks(s: &str) -> String {
    // Only remove complete fenced blocks that contain no non-whitespace content.
    // Leave all other content unchanged to avoid affecting partial streams.
    let lines: Vec<&str> = s.lines().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        let trimmed_start = line.trim_start();
        let fence_token = if trimmed_start.starts_with("```") {
            "```"
        } else if trimmed_start.starts_with("~~~") {
            "~~~"
        } else {
            ""
        };
        if !fence_token.is_empty() {
            // Find a matching closing fence on its own line.
            let mut j = i + 1;
            let mut has_content = false;
            let mut found_close = false;
            while j < lines.len() {
                let l = lines[j];
                if l.trim() == fence_token {
                    found_close = true;
                    break;
                }
                if !l.trim().is_empty() {
                    has_content = true;
                }
                j += 1;
            }
            if found_close && !has_content {
                // Drop i..=j and insert at most a single blank separator line.
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                i = j + 1;
                continue;
            }
            // Not an empty fenced block; emit as-is.
            out.push_str(line);
            out.push('\n');
            i += 1;
        } else {
            out.push_str(line);
            out.push('\n');
            i += 1;
        }
    }
    out
}
