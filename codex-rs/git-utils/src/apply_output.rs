//! Parsing of diagnostics emitted by Git apply.

use once_cell::sync::Lazy;
use regex::Regex;

fn unescape_c_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(next) = chars.next() else {
            out.push('\\');
            break;
        };
        match next {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' => out.push('\u{0008}'),
            'f' => out.push('\u{000C}'),
            'a' => out.push('\u{0007}'),
            'v' => out.push('\u{000B}'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            '0'..='7' => {
                let mut value = next.to_digit(8).unwrap_or(0);
                for _ in 0..2 {
                    match chars.peek() {
                        Some('0'..='7') => {
                            if let Some(digit) = chars.next() {
                                value = value * 8 + digit.to_digit(8).unwrap_or(0);
                            } else {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                if let Some(ch) = std::char::from_u32(value) {
                    out.push(ch);
                }
            }
            other => out.push(other),
        }
    }
    out
}

// ============ Parser ported from VS Code (TS) ============

/// Parse `git apply` output into applied/skipped/conflicted path groupings.
pub fn parse_git_apply_output(
    stdout: &str,
    stderr: &str,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let combined = [stdout, stderr]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<&str>>()
        .join("\n");

    let mut applied = std::collections::BTreeSet::new();
    let mut skipped = std::collections::BTreeSet::new();
    let mut conflicted = std::collections::BTreeSet::new();
    let mut last_seen_path: Option<String> = None;

    fn add(set: &mut std::collections::BTreeSet<String>, raw: &str) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        let first = trimmed.chars().next().unwrap_or('\0');
        let last = trimmed.chars().last().unwrap_or('\0');
        let unquoted = if (first == '"' || first == '\'') && last == first && trimmed.len() >= 2 {
            unescape_c_string(&trimmed[1..trimmed.len() - 1])
        } else {
            trimmed.to_string()
        };
        if !unquoted.is_empty() {
            set.insert(unquoted);
        }
    }

    static APPLIED_CLEAN: Lazy<Regex> =
        Lazy::new(|| regex_ci("^Applied patch(?: to)?\\s+(?P<path>.+?)\\s+cleanly\\.?$"));
    static APPLIED_CONFLICTS: Lazy<Regex> =
        Lazy::new(|| regex_ci("^Applied patch(?: to)?\\s+(?P<path>.+?)\\s+with conflicts\\.?$"));
    static APPLYING_WITH_REJECTS: Lazy<Regex> = Lazy::new(|| {
        regex_ci("^Applying patch\\s+(?P<path>.+?)\\s+with\\s+\\d+\\s+rejects?\\.{0,3}$")
    });
    static CHECKING_PATCH: Lazy<Regex> =
        Lazy::new(|| regex_ci("^Checking patch\\s+(?P<path>.+?)\\.\\.\\.$"));
    static UNMERGED_LINE: Lazy<Regex> = Lazy::new(|| regex_ci("^U\\s+(?P<path>.+)$"));
    static PATCH_FAILED: Lazy<Regex> =
        Lazy::new(|| regex_ci("^error:\\s+patch failed:\\s+(?P<path>.+?)(?::\\d+)?(?:\\s|$)"));
    static DOES_NOT_APPLY: Lazy<Regex> =
        Lazy::new(|| regex_ci("^error:\\s+(?P<path>.+?):\\s+patch does not apply$"));
    static THREE_WAY_START: Lazy<Regex> = Lazy::new(|| {
        regex_ci("^(?:Performing three-way merge|Falling back to three-way merge)\\.\\.\\.$")
    });
    static THREE_WAY_FAILED: Lazy<Regex> =
        Lazy::new(|| regex_ci("^Failed to perform three-way merge\\.\\.\\.$"));
    static FALLBACK_DIRECT: Lazy<Regex> =
        Lazy::new(|| regex_ci("^Falling back to direct application\\.\\.\\.$"));
    static LACKS_BLOB: Lazy<Regex> = Lazy::new(|| {
        regex_ci(
            "^(?:error: )?repository lacks the necessary blob to (?:perform|fall back on) 3-?way merge\\.?$",
        )
    });
    static INDEX_MISMATCH: Lazy<Regex> =
        Lazy::new(|| regex_ci("^error:\\s+(?P<path>.+?):\\s+does not match index\\b"));
    static NOT_IN_INDEX: Lazy<Regex> =
        Lazy::new(|| regex_ci("^error:\\s+(?P<path>.+?):\\s+does not exist in index\\b"));
    static ALREADY_EXISTS_WT: Lazy<Regex> = Lazy::new(|| {
        regex_ci("^error:\\s+(?P<path>.+?)\\s+already exists in (?:the )?working directory\\b")
    });
    static FILE_EXISTS: Lazy<Regex> =
        Lazy::new(|| regex_ci("^error:\\s+patch failed:\\s+(?P<path>.+?)\\s+File exists"));
    static RENAMED_DELETED: Lazy<Regex> =
        Lazy::new(|| regex_ci("^error:\\s+path\\s+(?P<path>.+?)\\s+has been renamed\\/deleted"));
    static CANNOT_APPLY_BINARY: Lazy<Regex> = Lazy::new(|| {
        regex_ci(
            "^error:\\s+cannot apply binary patch to\\s+['\\\"]?(?P<path>.+?)['\\\"]?\\s+without full index line$",
        )
    });
    static BINARY_DOES_NOT_APPLY: Lazy<Regex> = Lazy::new(|| {
        regex_ci("^error:\\s+binary patch does not apply to\\s+['\\\"]?(?P<path>.+?)['\\\"]?$")
    });
    static BINARY_INCORRECT_RESULT: Lazy<Regex> = Lazy::new(|| {
        regex_ci(
            "^error:\\s+binary patch to\\s+['\\\"]?(?P<path>.+?)['\\\"]?\\s+creates incorrect result\\b",
        )
    });
    static CANNOT_READ_CURRENT: Lazy<Regex> = Lazy::new(|| {
        regex_ci("^error:\\s+cannot read the current contents of\\s+['\\\"]?(?P<path>.+?)['\\\"]?$")
    });
    static SKIPPED_PATCH: Lazy<Regex> =
        Lazy::new(|| regex_ci("^Skipped patch\\s+['\\\"]?(?P<path>.+?)['\\\"]\\.$"));
    static CANNOT_MERGE_BINARY_WARN: Lazy<Regex> = Lazy::new(|| {
        regex_ci(
            "^warning:\\s*Cannot merge binary files:\\s+(?P<path>.+?)\\s+\\(ours\\s+vs\\.\\s+theirs\\)",
        )
    });

    for raw_line in combined.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // === "Checking patch <path>..." tracking ===
        if let Some(c) = CHECKING_PATCH.captures(line) {
            if let Some(m) = c.name("path") {
                last_seen_path = Some(m.as_str().to_string());
            }
            continue;
        }

        // === Status lines ===
        if let Some(c) = APPLIED_CLEAN.captures(line) {
            if let Some(m) = c.name("path") {
                add(&mut applied, m.as_str());
                let p = applied.iter().next_back().cloned();
                if let Some(p) = p {
                    conflicted.remove(&p);
                    skipped.remove(&p);
                    last_seen_path = Some(p);
                }
            }
            continue;
        }
        if let Some(c) = APPLIED_CONFLICTS.captures(line) {
            if let Some(m) = c.name("path") {
                add(&mut conflicted, m.as_str());
                let p = conflicted.iter().next_back().cloned();
                if let Some(p) = p {
                    applied.remove(&p);
                    skipped.remove(&p);
                    last_seen_path = Some(p);
                }
            }
            continue;
        }
        if let Some(c) = APPLYING_WITH_REJECTS.captures(line) {
            if let Some(m) = c.name("path") {
                add(&mut conflicted, m.as_str());
                let p = conflicted.iter().next_back().cloned();
                if let Some(p) = p {
                    applied.remove(&p);
                    skipped.remove(&p);
                    last_seen_path = Some(p);
                }
            }
            continue;
        }

        // === “U <path>” after conflicts ===
        if let Some(c) = UNMERGED_LINE.captures(line) {
            if let Some(m) = c.name("path") {
                add(&mut conflicted, m.as_str());
                let p = conflicted.iter().next_back().cloned();
                if let Some(p) = p {
                    applied.remove(&p);
                    skipped.remove(&p);
                    last_seen_path = Some(p);
                }
            }
            continue;
        }

        // === Early hints ===
        if PATCH_FAILED.is_match(line) || DOES_NOT_APPLY.is_match(line) {
            if let Some(c) = PATCH_FAILED
                .captures(line)
                .or_else(|| DOES_NOT_APPLY.captures(line))
                && let Some(m) = c.name("path")
            {
                add(&mut skipped, m.as_str());
                last_seen_path = Some(m.as_str().to_string());
            }
            continue;
        }

        // === Ignore narration ===
        if THREE_WAY_START.is_match(line) || FALLBACK_DIRECT.is_match(line) {
            continue;
        }

        // === 3-way failed entirely; attribute to last_seen_path ===
        if THREE_WAY_FAILED.is_match(line) || LACKS_BLOB.is_match(line) {
            if let Some(p) = last_seen_path.clone() {
                add(&mut skipped, &p);
                applied.remove(&p);
                conflicted.remove(&p);
            }
            continue;
        }

        // === Skips / I/O problems ===
        if let Some(c) = INDEX_MISMATCH
            .captures(line)
            .or_else(|| NOT_IN_INDEX.captures(line))
            .or_else(|| ALREADY_EXISTS_WT.captures(line))
            .or_else(|| FILE_EXISTS.captures(line))
            .or_else(|| RENAMED_DELETED.captures(line))
            .or_else(|| CANNOT_APPLY_BINARY.captures(line))
            .or_else(|| BINARY_DOES_NOT_APPLY.captures(line))
            .or_else(|| BINARY_INCORRECT_RESULT.captures(line))
            .or_else(|| CANNOT_READ_CURRENT.captures(line))
            .or_else(|| SKIPPED_PATCH.captures(line))
        {
            if let Some(m) = c.name("path") {
                add(&mut skipped, m.as_str());
                let p_now = skipped.iter().next_back().cloned();
                if let Some(p) = p_now {
                    applied.remove(&p);
                    conflicted.remove(&p);
                    last_seen_path = Some(p);
                }
            }
            continue;
        }

        // === Warnings that imply conflicts ===
        if let Some(c) = CANNOT_MERGE_BINARY_WARN.captures(line) {
            if let Some(m) = c.name("path") {
                add(&mut conflicted, m.as_str());
                let p = conflicted.iter().next_back().cloned();
                if let Some(p) = p {
                    applied.remove(&p);
                    skipped.remove(&p);
                    last_seen_path = Some(p);
                }
            }
            continue;
        }
    }

    // Final precedence: conflicts > applied > skipped
    for p in conflicted.iter() {
        applied.remove(p);
        skipped.remove(p);
    }
    for p in applied.iter() {
        skipped.remove(p);
    }

    (
        applied.into_iter().collect(),
        skipped.into_iter().collect(),
        conflicted.into_iter().collect(),
    )
}

fn regex_ci(pat: &str) -> Regex {
    Regex::new(&format!("(?i){pat}")).unwrap_or_else(|e| panic!("invalid regex: {e}"))
}
