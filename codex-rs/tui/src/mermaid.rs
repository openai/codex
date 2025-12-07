use std::collections::HashMap;

use lazy_static::lazy_static;
use regex::Captures;
use regex::Regex;

type FixFn = Box<dyn Fn(&str) -> String + Send + Sync>;

struct Issue {
    line_no: usize,
    #[allow(dead_code)]
    start: usize,
    #[allow(dead_code)]
    end: usize,
    #[allow(dead_code)]
    message: String,
    fix: Option<FixFn>,
}

impl Issue {
    fn new(
        line_no: usize,
        start: usize,
        end: usize,
        message: impl Into<String>,
        fix: FixFn,
    ) -> Self {
        Self {
            line_no,
            start,
            end,
            message: message.into(),
            fix: Some(fix),
        }
    }
}

struct MermaidLinter {
    lines: Vec<String>,
}

impl MermaidLinter {
    fn new(source: &str) -> Self {
        let lines = if source.is_empty() {
            Vec::new()
        } else {
            source
                .split('\n')
                .map(std::string::ToString::to_string)
                .collect()
        };
        Self { lines }
    }

    fn lint(&mut self) -> Vec<Issue> {
        let mut issues: Vec<Issue> = Vec::new();
        let mut in_pie = false;
        let mut in_sequence = false;
        let mut in_diagram = false;
        let mut pending_updates: HashMap<usize, String> = HashMap::new();

        let mut lines_copy = self.lines.clone();
        for (idx, line) in lines_copy.iter().enumerate() {
            let line_no = idx + 1;
            let trimmed = line.trim();
            let lowered = trimmed.to_lowercase();

            let arrow_matches: Vec<_> = ARROW_RE.find_iter(line).collect();
            let label_spans = compute_label_spans(line);
            let filtered_arrows: Vec<_> = arrow_matches
                .iter()
                .cloned()
                .filter(|m| {
                    let start = m.start();
                    let end = m.end();
                    !label_spans.iter().any(|(a, b)| start >= *a && end <= *b)
                        && !is_within_double_quotes(line, start, end)
                })
                .collect();

            if filtered_arrows.is_empty()
                && in_diagram
                && !in_sequence
                && line.contains("--|")
                && let Some((lhs, rest)) = line.split_once("--|")
                && let Some((label, rhs)) = rest.split_once('|')
            {
                let left = wrap_node_if_plain(lhs.trim());
                let right = wrap_node_if_plain(rhs.trim());
                if !left.is_empty() && !right.is_empty() {
                    let sanitized_label = sanitize_label_text(label);
                    let normalized_label = if sanitized_label.is_empty() {
                        label.trim().to_string()
                    } else {
                        sanitized_label
                    };
                    let replacement = format!("{left} -->|{normalized_label}| {right}");
                    issues.push(Issue::new(
                        line_no,
                        0,
                        line.len(),
                        "Normalized labeled edge to use '-->' and sanitized label.",
                        Box::new(move |_| replacement.clone()),
                    ));
                    continue;
                }
            }
            if filtered_arrows.len() > 1 {
                let replacement = filtered_arrows
                    .iter()
                    .enumerate()
                    .filter_map(|(i, m)| {
                        let start = if i == 0 {
                            0
                        } else {
                            filtered_arrows[i - 1].end()
                        };
                        let end = filtered_arrows
                            .get(i + 1)
                            .map(regex::Match::start)
                            .unwrap_or_else(|| line.len());
                        let source = line[start..m.start()].trim();
                        let target = line[m.end()..end].trim();
                        if source.is_empty() && target.is_empty() {
                            return None;
                        }
                        let left = wrap_node_if_plain(source);
                        let right = wrap_node_if_plain(target);
                        let arrow = m.as_str().trim();
                        Some(format!("{left} {arrow} {right}").trim().to_string())
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                issues.push(Issue::new(
                    line_no,
                    0,
                    line.len(),
                    "Multiple arrows on one line; splitting into separate edges.",
                    Box::new(move |_| replacement.clone()),
                ));
                continue;
            }
            if let Some(single_arrow) = filtered_arrows.first() {
                let right_side = line[single_arrow.end()..].trim();
                let pipe_count = right_side.matches('|').count();
                if pipe_count == 1 && right_side.starts_with('|') {
                    let tail = right_side.trim_start_matches('|').trim();
                    if let Some((label, target)) = tail.split_once(char::is_whitespace) {
                        let cleaned_label = label.trim();
                        let cleaned_target = target.trim();
                        if !cleaned_label.is_empty() && !cleaned_target.is_empty() {
                            let left = wrap_node_if_plain(line[..single_arrow.start()].trim());
                            let arrow_text = single_arrow.as_str().trim();
                            let replacement =
                                format!("{left} {arrow_text}|{cleaned_label}| {cleaned_target}");
                            issues.push(Issue::new(
                                line_no,
                                0,
                                line.len(),
                                "Arrow label missing closing pipe; inserted and separated target.",
                                Box::new(move |_| replacement.clone()),
                            ));
                            continue;
                        }
                    }
                }

                let tail = &line[single_arrow.end()..];
                if let Some(first_pipe_rel) = tail.find('|') {
                    let after_first = &tail[first_pipe_rel + 1..];
                    if let Some(second_pipe_rel) = after_first.find('|') {
                        let label_start = single_arrow.end() + first_pipe_rel + 1;
                        let label_end = label_start + second_pipe_rel;
                        let label_text = &line[label_start..label_end];
                        if label_text.contains(['(', ')', '"']) {
                            let sanitized = sanitize_label_text(label_text);
                            if !sanitized.is_empty() && sanitized != label_text {
                                issues.push(Issue::new(
                                    line_no,
                                    label_start,
                                    label_end,
                                    "Sanitized edge label containing parentheses/quotes.",
                                    make_replace_span(label_start, label_end, sanitized),
                                ));
                            }
                        }
                    }
                }
            }

            if lowered.starts_with("pie") {
                in_pie = true;
                in_diagram = true;
            }
            if lowered.starts_with("sequencediagram") {
                in_sequence = true;
                in_diagram = true;
            } else if lowered.starts_with("graph")
                || lowered.starts_with("flowchart")
                || lowered.starts_with("classdiagram")
                || lowered.starts_with("erdiagram")
                || lowered.starts_with("gantt")
            {
                in_sequence = false;
                in_pie = false;
                in_diagram = true;
            }

            if STYLE_RE.is_match(line) {
                issues.push(Issue::new(
                    line_no,
                    0,
                    line.len(),
                    "Unsupported 'style' directive; removing line.",
                    Box::new(|_| String::new()),
                ));
                continue;
            }

            if in_diagram
                && (lowered.starts_with("title ")
                    || lowered == "title"
                    || lowered.starts_with("title:"))
            {
                issues.push(Issue::new(
                    line_no,
                    0,
                    line.len(),
                    "Mermaid titles are not supported; removing line.",
                    Box::new(|_| String::new()),
                ));
                continue;
            }

            for (pos, _) in line.match_indices('\t') {
                issues.push(Issue::new(
                    line_no,
                    pos,
                    pos + 1,
                    "Tab character found; use spaces instead.",
                    Box::new(|line_text: &str| line_text.replacen('\t', "  ", 1)),
                ));
            }

            if line.trim_end() != *line {
                let trimmed_line = line.trim_end().to_string();
                issues.push(Issue::new(
                    line_no,
                    trimmed_line.len(),
                    line.len(),
                    "Trailing whitespace.",
                    Box::new(move |_| trimmed_line.clone()),
                ));
            }

            if !in_sequence {
                for arrow in &filtered_arrows {
                    let start = arrow.start();
                    let end = arrow.end();
                    let arrow_text = arrow.as_str();
                    let normalized = arrow_text.trim();
                    if normalized.chars().any(|c| !matches!(c, '-' | '>')) {
                        continue;
                    }
                    if normalized != "-->" {
                        let message =
                            format!("Inconsistent arrow style '{arrow_text}'; use '-->'.");
                        issues.push(Issue::new(
                            line_no,
                            start,
                            end,
                            message,
                            make_replace_span(start, end, "-->".to_string()),
                        ));
                    }
                }
            }

            if !in_diagram {
                for arrow in ARROW_RE.find_iter(line) {
                    let start = arrow.start();
                    let end = arrow.end();
                    let lhs = &line[..start];
                    let rhs = &line[end..];
                    let left_tok = lhs
                        .split_whitespace()
                        .last()
                        .map(str::to_string)
                        .unwrap_or_default();
                    let right_tok = rhs
                        .split_whitespace()
                        .next()
                        .map(str::to_string)
                        .unwrap_or_default();

                    for node_tok in [left_tok, right_tok] {
                        if node_tok.is_empty() || NODE_ID_VALID_RE.is_match(&node_tok) {
                            continue;
                        }
                        if let Some(span_start) = line.rfind(&node_tok) {
                            let span_end = span_start + node_tok.len();
                            let sanitized = sanitize_node_id(&node_tok);
                            issues.push(Issue::new(
                                line_no,
                                span_start,
                                span_end,
                                "Node identifier should be lower_snake_case.",
                                make_replace_span(span_start, span_end, sanitized),
                            ));
                        }
                    }
                }
            }

            if in_sequence {
                let first_arrow = SEQ_ARROW_RE.find(line).or_else(|| ARROW_RE.find(line));
                if let Some(arrow_match) = first_arrow {
                    let mut before = line[..arrow_match.start()].to_string();
                    let mut after = line[arrow_match.end()..].to_string();
                    let arrow_text = arrow_match.as_str();
                    let mut changed = false;

                    if let Some(caps) = SEQ_SENDER_UNDERSCORE_RE.captures(&before)
                        && let (Some(group), Some(full)) = (caps.get(1), caps.get(0))
                    {
                        let replacement = group.as_str().trim_end_matches('_').to_string();
                        let mut new_before = before[..group.start()].to_string();
                        new_before.push_str(&replacement);
                        new_before.push_str(&before[full.end()..]);
                        before = new_before;
                        changed = true;
                    }

                    if let Some(caps) = SEQ_RECEIVER_UNDERSCORE_RE.captures(&after) {
                        if let (Some(recv), Some(rest)) = (caps.get(2), caps.get(4)) {
                            after = format!("{}: {}", recv.as_str(), rest.as_str().trim_start());
                            changed = true;
                        }
                    } else if let Some(caps) = SEQ_RECEIVER_MISSING_COLON_RE.captures(&after)
                        && !after.trim_start().starts_with(':')
                        && let (Some(recv), Some(rest)) = (caps.get(2), caps.get(4))
                    {
                        after = format!("{}: {}", recv.as_str(), rest.as_str().trim_start());
                        changed = true;
                    }

                    if changed {
                        let updated = format!("{before}{arrow_text}{after}");
                        pending_updates.insert(idx, updated);
                    }
                }

                if let Some(arrow_match) = SEQ_ARROW_RE.find(line).or_else(|| ARROW_RE.find(line))
                    && let Some(rel_colon) = line[arrow_match.end()..].find(':')
                {
                    let colon_pos = arrow_match.end() + rel_colon;
                    if line[colon_pos + 1..].contains(';') {
                        issues.push(Issue::new(
                            line_no,
                            colon_pos + 1,
                            line.len(),
                            "Semicolons in sequence message; use commas or split lines.",
                            make_replace_after_colon(colon_pos, ';', ','),
                        ));
                    }
                }
            }

            if in_diagram && !in_pie && !in_sequence {
                for captures in SQUARE_LABEL_RE.captures_iter(line) {
                    if let Some(span) = captures.get(1) {
                        let raw = span.as_str();
                        if is_already_quoted(raw) {
                            continue;
                        }
                        let start_idx = span.start();
                        let end_idx = span.end();
                        let replacement = format!("\"{}\"", raw.replace('"', "'"));
                        issues.push(Issue::new(
                            line_no,
                            start_idx,
                            end_idx,
                            "Quote node label inside [] to allow punctuation.",
                            make_replace_span(start_idx, end_idx, replacement),
                        ));
                    }
                }

                for captures in PAR2_LABEL_RE.captures_iter(line) {
                    if let Some(mat) = captures.get(1) {
                        if is_within_double_quotes(line, mat.start(), mat.end()) {
                            continue;
                        }
                        let raw = mat.as_str();
                        if is_already_quoted(raw) {
                            continue;
                        }
                        let start_idx = mat.start();
                        let end_idx = mat.end();
                        let replacement = format!("\"{}\"", raw.replace('"', "'"));
                        issues.push(Issue::new(
                            line_no,
                            start_idx,
                            end_idx,
                            "Quote node label inside (( )) to allow punctuation.",
                            make_replace_span(start_idx, end_idx, replacement),
                        ));
                    }
                }

                for captures in PAR1_LABEL_RE.captures_iter(line) {
                    if let Some(mat) = captures.get(1) {
                        if is_within_double_quotes(line, mat.start(), mat.end()) {
                            continue;
                        }
                        let raw = mat.as_str();
                        if is_already_quoted(raw) {
                            continue;
                        }
                        let start_idx = mat.start();
                        let end_idx = mat.end();
                        let replacement = format!("\"{}\"", raw.replace('"', "'"));
                        issues.push(Issue::new(
                            line_no,
                            start_idx,
                            end_idx,
                            "Quote node label inside () to allow punctuation.",
                            make_replace_span(start_idx, end_idx, replacement),
                        ));
                    }
                }
            }

            if in_pie && let Some(caps) = PIE_LINE_RE.captures(line) {
                let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
                let label = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
                let value = caps.get(3).map(|m| m.as_str()).unwrap_or("").to_string();

                let mut fixed_label = label.clone();
                for _ in 0..3 {
                    let updated = PIE_INNER_QUOTE_RE
                        .replace_all(&fixed_label, "$1$2$3")
                        .into_owned();
                    if updated == fixed_label {
                        break;
                    }
                    fixed_label = updated;
                }

                if fixed_label != label {
                    let replacement_line = format!("{indent}\"{fixed_label}\": {value}");
                    issues.push(Issue::new(
                        line_no,
                        0,
                        line.len(),
                        "Pie label contains quoted numeric count; removing inner quotes.",
                        Box::new(move |_| replacement_line.clone()),
                    ));
                }
            }

            if let Some(caps) = NODE_ID_RE.captures(line)
                && let Some(group) = caps.get(1)
            {
                let bad_id = group.as_str();
                if !NODE_ID_VALID_RE.is_match(bad_id) {
                    let sanitized = sanitize_node_id(bad_id);
                    issues.push(Issue::new(
                        line_no,
                        group.start(),
                        group.end(),
                        "Node identifier should be lower_snake_case.",
                        make_replace_span(group.start(), group.end(), sanitized),
                    ));
                }
            } else if !in_sequence && let Some(arrow) = filtered_arrows.first() {
                let left = line[..arrow.start()].trim();
                let right = line[arrow.end()..].trim();
                let wrapped_left = wrap_node_if_plain(left);
                let wrapped_right = wrap_node_if_plain(right);
                if wrapped_left != left || wrapped_right != right {
                    let arrow_text = arrow.as_str().trim();
                    let replacement = format!("{wrapped_left} {arrow_text} {wrapped_right}")
                        .trim()
                        .to_string();
                    issues.push(Issue::new(
                        line_no,
                        0,
                        line.len(),
                        "Wrapped node names with punctuation or spaces in quotes.",
                        Box::new(move |_| replacement.clone()),
                    ));
                }
            }
        }

        for (idx, updated) in pending_updates {
            if idx < lines_copy.len() {
                lines_copy[idx] = updated;
            }
        }

        self.lines = lines_copy;
        issues
    }

    fn apply_fixes(&mut self, mut issues: Vec<Issue>) -> usize {
        const MAX_PASSES: usize = 10;
        let mut passes = 0usize;

        while !issues.is_empty() && passes < MAX_PASSES {
            let remaining = self.apply_fixes_inner(&issues);
            passes += 1;
            if remaining == 0 {
                break;
            }
            issues = self.lint();
        }

        passes
    }

    fn apply_fixes_inner(&mut self, issues: &[Issue]) -> usize {
        let mut issue_map: HashMap<usize, Vec<&Issue>> = HashMap::new();
        for issue in issues {
            issue_map.entry(issue.line_no).or_default().push(issue);
        }

        let mut new_lines: Vec<String> = Vec::with_capacity(self.lines.len());
        let mut unfixed = 0usize;

        for (idx, line) in self.lines.iter().enumerate() {
            let line_no = idx + 1;
            let Some(issues_on_line) = issue_map.get(&line_no) else {
                new_lines.push(line.clone());
                continue;
            };

            if issues_on_line.len() > 1 {
                let Some((first_issue, rest)) = issues_on_line.split_first() else {
                    continue;
                };
                if let Some(fix) = &first_issue.fix {
                    let fixed = fix(line);
                    if !fixed.is_empty() {
                        new_lines.push(fixed);
                    }
                } else {
                    unfixed += 1;
                    new_lines.push(line.clone());
                }
                unfixed += rest.len();
                continue;
            }

            let issue = issues_on_line[0];
            if let Some(fix) = &issue.fix {
                let fixed_line = fix(line);
                if !fixed_line.is_empty() {
                    new_lines.push(fixed_line);
                }
            } else {
                unfixed += 1;
                new_lines.push(line.clone());
            }
        }

        self.lines = new_lines;
        unfixed
    }
}

fn must_compile(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|err| panic!("invalid regex {pattern}: {err}"))
}

lazy_static! {
    static ref STYLE_RE: Regex = must_compile(r"(?i)^\s*style\b");
    static ref ARROW_RE: Regex = must_compile(r"-{1,}[^-]*>");
    static ref SEQ_ARROW_RE: Regex = must_compile(r"-{1,2}(?:>>|>)");
    static ref NODE_ID_RE: Regex = must_compile(r"^\s*([a-zA-Z0-9_]+)\s*[\[(]");
    static ref NODE_ID_VALID_RE: Regex = must_compile(r"^[A-Za-z0-9_]+$");
    static ref SQUARE_LABEL_RE: Regex = must_compile(r"[A-Za-z0-9_]+\s*\[(.*?)\]");
    static ref PAR2_LABEL_RE: Regex = must_compile(r"[A-Za-z0-9_]+\s*\(\((.*?)\)\)");
    static ref PAR1_LABEL_RE: Regex = must_compile(r"[A-Za-z0-9_]+\s*\(([^()]*?)\)");
    static ref SEQ_SENDER_UNDERSCORE_RE: Regex = must_compile(r"([A-Za-z0-9_]+)_\s*$");
    static ref SEQ_RECEIVER_UNDERSCORE_RE: Regex =
        must_compile(r"^(\s*([A-Za-z0-9_]+))_(\s*)(.*)$");
    static ref SEQ_RECEIVER_MISSING_COLON_RE: Regex =
        must_compile(r"^(\s*([A-Za-z0-9_]+))(\s+)(.*)$");
    static ref PIE_LINE_RE: Regex = must_compile(r#"^(\s*)"(.+)"\s*:\s*([0-9]+(?:\.[0-9]+)?)\s*$"#);
    static ref PIE_INNER_QUOTE_RE: Regex =
        must_compile(r#"([\(\[])\s*['"](\d+(?:\.\d+)?)['"]\s*([\)\]])"#);
    static ref MERMAID_FENCE_RE: Regex = must_compile(r"(?is)```mermaid(.*?)```");
    static ref GENERIC_FENCE_RE: Regex = must_compile(r"(?is)```([a-zA-Z0-9_+-]*)\n(.*?)```");
    static ref HEADER_RE: Regex =
        must_compile(r"(?i)^\s*(flowchart|graph|sequenceDiagram|classDiagram|erDiagram|gantt)\b");
    static ref HEADER_TITLE_SAME_LINE_RE: Regex = must_compile(
        r"(?im)^(?P<indent>\s*)(?P<keyword>flowchart|graph)\s+(?P<dir>TB|TD|LR|RL|BT)\s+title\s+(?P<title>.+)$",
    );
}

fn wrap_node_if_plain(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains(['[', '(']) || NODE_ID_VALID_RE.is_match(trimmed) {
        return trimmed.to_string();
    }
    let id = sanitize_node_id(trimmed);
    let safe_label = trimmed.replace('"', "'");
    format!("{id}[\"{safe_label}\"]")
}

fn sanitize_node_id(value: &str) -> String {
    let replaced = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    replaced.to_lowercase()
}

fn make_replace_span(start: usize, end: usize, replacement: String) -> FixFn {
    Box::new(move |line: &str| {
        let mut result = String::with_capacity(line.len() - (end - start) + replacement.len());
        result.push_str(&line[..start]);
        result.push_str(&replacement);
        result.push_str(&line[end..]);
        result
    })
}

fn make_replace_after_colon(colon_pos: usize, find: char, replace_with: char) -> FixFn {
    Box::new(move |line: &str| {
        let mut result = String::with_capacity(line.len());
        result.push_str(&line[..=colon_pos]);
        let tail = line[colon_pos + 1..].replace(find, &replace_with.to_string());
        result.push_str(&tail);
        result
    })
}

fn sanitize_label_text(raw: &str) -> String {
    let cleaned = raw.replace(['(', ')', '"'], " ");
    cleaned
        .split_whitespace()
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn compute_label_spans(line: &str) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for caps in SQUARE_LABEL_RE.captures_iter(line) {
        if let Some(inner) = caps.get(1) {
            spans.push((inner.start(), inner.end()));
        }
    }
    for caps in PAR2_LABEL_RE.captures_iter(line) {
        if let Some(inner) = caps.get(1) {
            spans.push((inner.start(), inner.end()));
        }
    }
    for caps in PAR1_LABEL_RE.captures_iter(line) {
        if let Some(inner) = caps.get(1) {
            spans.push((inner.start(), inner.end()));
        }
    }
    spans
}

fn is_already_quoted(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
}

fn is_within_double_quotes(line: &str, start: usize, end: usize) -> bool {
    let mut in_quote = false;
    let mut quote_start = 0usize;
    let mut spans: Vec<(usize, usize)> = Vec::new();

    for (idx, ch) in line.char_indices() {
        if ch == '"' {
            if !in_quote {
                in_quote = true;
                quote_start = idx;
            } else {
                spans.push((quote_start, idx));
                in_quote = false;
            }
        }
    }

    spans.iter().any(|(a, b)| start >= *a && end <= *b)
}

fn normalize_header_titles(source: &str) -> String {
    HEADER_TITLE_SAME_LINE_RE
        .replace_all(source, |caps: &Captures| {
            let indent = caps.name("indent").map(|m| m.as_str()).unwrap_or("");
            let keyword = caps
                .name("keyword")
                .map(|m| m.as_str())
                .unwrap_or("flowchart");
            let dir = caps.name("dir").map(|m| m.as_str()).unwrap_or("TD");
            format!("{indent}{keyword} {dir}")
        })
        .into_owned()
}

fn ensure_mermaid_header(source: &str) -> String {
    let mut lines = source.lines();
    let first_non_empty = lines.find(|line| !line.trim().is_empty());
    if let Some(first) = first_non_empty
        && HEADER_RE.is_match(first.trim())
    {
        return source.to_string();
    }
    if source.trim().is_empty() {
        return "flowchart TD".to_string();
    }
    let mut out = String::new();
    out.push_str("flowchart TD\n");
    out.push_str(source.trim_start_matches('\n'));
    out
}

fn lint_and_wrap(code: &str) -> String {
    let ensured = ensure_mermaid_header(code);
    let normalized = normalize_header_titles(&ensured);
    let mut linter = MermaidLinter::new(&normalized);
    let issues = linter.lint();
    linter.apply_fixes(issues);
    let fixed = if linter.lines.is_empty() {
        String::new()
    } else {
        linter.lines.join("\n")
    };
    format!("```mermaid\n{fixed}\n```")
}

pub(crate) fn fix_mermaid_blocks(input: &str) -> String {
    if input.trim().is_empty() {
        return input.to_string();
    }

    let after_fenced = MERMAID_FENCE_RE
        .replace_all(input, |caps: &Captures| {
            let Some(_full_match) = caps.get(0) else {
                return caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string();
            };
            let body = caps
                .get(1)
                .map(|m| m.as_str())
                .unwrap_or("")
                .trim_matches('\n');

            // Always left-align mermaid fences to avoid excessive indentation in rendered diagrams.
            lint_and_wrap(body)
        })
        .into_owned();

    let after_generic = GENERIC_FENCE_RE
        .replace_all(&after_fenced, |caps: &Captures| {
            let Some(_full_match) = caps.get(0) else {
                return caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string();
            };
            let lang = caps
                .get(1)
                .map(|m| m.as_str())
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if lang == "mermaid" {
                return caps
                    .get(0)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
            }
            let body = caps
                .get(2)
                .map(|m| m.as_str())
                .unwrap_or("")
                .trim_matches('\n');
            let head = body.lines().next().unwrap_or("").trim().to_lowercase();
            if [
                "flowchart",
                "graph",
                "sequencediagram",
                "classdiagram",
                "erdiagram",
                "gantt",
            ]
            .iter()
            .any(|prefix| head.starts_with(prefix))
            {
                lint_and_wrap(body)
            } else {
                caps.get(0)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default()
            }
        })
        .into_owned();

    let lines: Vec<String> = after_generic
        .split('\n')
        .map(std::string::ToString::to_string)
        .collect();

    if lines.is_empty() {
        return after_generic;
    }

    let mut out_lines: Vec<String> = Vec::new();
    let mut in_code_block = false;
    let mut idx = 0usize;

    while idx < lines.len() {
        let line = &lines[idx];
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            out_lines.push(line.clone());
            idx += 1;
            continue;
        }

        if !in_code_block && HEADER_RE.is_match(trimmed) {
            let start = idx;
            let mut end = idx;
            while end < lines.len() {
                let current = lines[end].trim();
                if current.starts_with("```") || current.is_empty() {
                    break;
                }
                end += 1;
            }
            let block = lines[start..end].join("\n");
            out_lines.push(lint_and_wrap(block.trim_matches('\n')));
            idx = end;
            // Always add a blank line separator after a mermaid block to avoid
            // back-to-back fenced blocks which some renderers mishandle.
            if idx < lines.len() {
                if lines[idx].trim().is_empty() {
                    out_lines.push(lines[idx].clone());
                    idx += 1;
                } else {
                    out_lines.push(String::new());
                }
            }
            continue;
        }

        out_lines.push(line.clone());
        idx += 1;
    }

    out_lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::fix_mermaid_blocks;
    use pretty_assertions::assert_eq;
    use regex::Regex;

    #[test]
    fn flowchart_nodes_are_quoted_even_when_unfenced() {
        let raw = [
            "flowchart LR",
            "  A[Caller Service or SDK] --> B[ekm_client Encryptor]",
            "  B --> C[KeyProviderDefault (HTTP)]",
            "  C --> D[EKM FastAPI Service]",
            "  D --> E[Provider Selector]",
            "  E --> F[Cloud KMS (AWS/GCP/Azure)]",
            "  B --> G[ekm_client_cpp V1Header build/parse]",
            "  B --> H[Tink AEAD (streaming/non-streaming)]",
        ]
        .join("\n");
        let fixed = fix_mermaid_blocks(&raw);
        assert!(fixed.contains(r#"A["Caller Service or SDK"]"#));
        assert!(fixed.contains(r#"B["ekm_client Encryptor"]"#));
        assert!(fixed.contains(r#"C["KeyProviderDefault (HTTP)"]"#));
        assert!(fixed.contains(r#"D["EKM FastAPI Service"]"#));
        assert!(fixed.contains(r#"E["Provider Selector"]"#));
        assert!(fixed.contains(r#"F["Cloud KMS (AWS/GCP/Azure)"]"#));
        assert!(fixed.contains(r#"G["ekm_client_cpp V1Header build/parse"]"#));
        assert!(fixed.contains(r#"H["Tink AEAD (streaming/non-streaming)"]"#));
    }

    #[test]
    fn sequence_semicolons_removed_in_messages() {
        let raw = "```mermaid\nsequenceDiagram\n  Ingress->>Ingress: Sanitize logs; reject non-HTTPS with 403\n```";
        let fixed = fix_mermaid_blocks(raw);
        let message_re = Regex::new(r"Ingress->>Ingress:\s*(.*)").unwrap();
        let msg = message_re
            .captures(&fixed)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str())
            .unwrap_or_default();
        assert!(!msg.contains(';'));
        assert!(msg.contains(','));
    }

    #[test]
    fn unfenced_pie_chart_is_not_wrapped() {
        let raw = r#"pie
  "High ("12")": 12
  "Medium ("39")": 39
  "Low ("20")": 20
"#;
        let fixed = fix_mermaid_blocks(raw);
        assert!(!fixed.contains("```mermaid"));
    }

    #[test]
    fn pie_chart_inner_quotes_removed_when_fenced() {
        let raw = r#"```mermaid
pie
  "High ("12")": 12
  "Medium ("39")": 39
  "Low ("20")": 20
```"#;
        let fixed = fix_mermaid_blocks(raw);
        assert!(fixed.contains(r#""High (12)": 12"#));
        assert!(fixed.contains(r#""Medium (39)": 39"#));
        assert!(fixed.contains(r#""Low (20)": 20"#));
    }

    #[test]
    fn flowchart_paren_labels_are_quoted() {
        let raw = "```mermaid\nflowchart LR\n  A((Start node)) --> B(Account)\n```";
        let fixed = fix_mermaid_blocks(raw);
        assert!(fixed.contains(r#"A(("Start node"))"#));
        assert!(fixed.contains(r#"B("Account")"#));
    }

    #[test]
    fn graph_nodes_are_quoted_even_when_unfenced() {
        let raw = "graph LR\n  A[Client App] --> B[API Server]\n  B --> C[DB (primary)]\n";
        let fixed = fix_mermaid_blocks(raw);
        assert!(fixed.contains(r#"A["Client App"]"#));
        assert!(fixed.contains(r#"B["API Server"]"#));
        assert!(fixed.contains(r#"C["DB (primary)"]"#));
    }

    #[test]
    fn flowchart_quotes_and_preserves_inner_arrows() {
        let raw = [
            "flowchart LR",
            "  ClientApp[Client App - React] --> CalpicoState[Calpico State (signals + React Query)]",
            "  CalpicoState --> APIServer[API Server (/api/calpico)]",
            "  CalpicoState --> WebSocket[WebSocket Events]",
            "  CalpicoState --> FileService[File Upload Service]",
            "  APIServer --> CalpicoUtils[calpico_utils (post -> messages)]",
            "  CalpicoUtils --> APIServer",
            "  ClientApp --> UIComponents[UI Components (Composer, Thread, Sidebar)]",
            "  WebSocket --> CalpicoState",
        ]
        .join("\n");
        let fixed = fix_mermaid_blocks(&raw);
        assert!(fixed.contains(r#"ClientApp["Client App - React"]"#));
        assert!(fixed.contains(r#"CalpicoState["Calpico State (signals + React Query)"]"#));
        assert!(fixed.contains(r#"APIServer["API Server (/api/calpico)"]"#));
        assert!(fixed.contains(r#"UIComponents["UI Components (Composer, Thread, Sidebar)"]"#));
        assert!(fixed.contains(r#"CalpicoUtils["calpico_utils (post -> messages)"]"#));
        assert!(fixed.contains("post -> messages"));
        assert!(!fixed.contains("post --> messages"));
    }

    #[test]
    fn sequence_message_quotes_preserved() {
        let raw = r#"```mermaid
sequenceDiagram
  API->>FileMgr: sanitize & upload files ("if any")
```"#;
        let fixed = fix_mermaid_blocks(raw);
        assert!(fixed.contains(r#"API->>FileMgr: sanitize & upload files ("if any")"#));
    }

    #[test]
    fn sequence_receiver_underscore_repaired_to_colon() {
        let raw = r#"```mermaid
sequenceDiagram
  ClientApp->>APIServer_ POST /api/calpico/rooms/{id}/messages
  APIServer->>MessageWriter_ validate membership, persist message
  MessageWriter-->>APIServer_ message record
```"#;
        let fixed = fix_mermaid_blocks(raw);
        assert!(fixed.contains("ClientApp->>APIServer: POST /api/calpico/rooms/{id}/messages"));
        assert!(fixed.contains("APIServer->>MessageWriter: validate membership, persist message"));
        assert!(fixed.contains("MessageWriter-->>APIServer: message record"));
    }

    #[test]
    fn edge_labels_with_parens_and_quotes_are_sanitized() {
        let raw = "```mermaid\nflowchart TD\n  B --|HTTP(\"S\") + cookies/CSRF| W\n```";
        let fixed = fix_mermaid_blocks(raw);
        assert!(fixed.contains(r#"B -->|HTTP S + cookies/CSRF| W"#));
    }

    #[test]
    fn header_title_on_same_line_is_removed() {
        let raw = "```mermaid\nflowchart TD title Component request flow - end-to-end platform\n  Client[\"Tenant client / automation workflow\"] --> ChatService[\"packages/chat-service\"]\n```";
        let fixed = fix_mermaid_blocks(raw);
        assert!(fixed.contains("flowchart TD"));
        assert!(
            fixed.contains(
                r#"Client["Tenant client / automation workflow"] --> ChatService["packages/chat-service"]"#
            )
        );
        assert!(
            !fixed
                .to_lowercase()
                .contains("title component request flow - end-to-end platform")
        );
    }

    #[test]
    fn standalone_title_directive_is_removed() {
        let raw = [
            "flowchart LR",
            "title Planner service data flow",
            "  User -->|HTTP + Authorization| Fastify",
            r#"  Fastify -->|auth hook| APIGateway["/API Gateway /auth/me/"]"#,
            r#"  Fastify -->|handlers| Supabase["\(Postgres\)"]"#,
            "  Fastify -->|AI prompts| OpenAI",
            "  Supabase --> Supabase",
        ]
        .join("\n");
        let fixed = fix_mermaid_blocks(&raw);
        assert!(fixed.contains("flowchart LR"));
        assert!(
            !fixed
                .to_lowercase()
                .contains("title planner service data flow")
        );
    }

    #[test]
    fn round_trip_no_mermaid_returns_input() {
        let raw = "This markdown has no mermaid.\n\n```rust\nfn main() {}\n```\n";
        let fixed = fix_mermaid_blocks(raw);
        assert_eq!(raw, fixed);
    }
}
