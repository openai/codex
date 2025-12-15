use crate::config::types::GraphitiScope;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GraphitiEpisodeKind {
    Decision,
    LessonLearned,
    Preference,
    Procedure,
    TaskUpdate,
    Terminology,
}

impl GraphitiEpisodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Decision => "decision",
            Self::LessonLearned => "lesson_learned",
            Self::Preference => "preference",
            Self::Procedure => "procedure",
            Self::TaskUpdate => "task_update",
            Self::Terminology => "terminology",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphitiMemoryDirective {
    pub kind: GraphitiEpisodeKind,
    pub scope: GraphitiScope,
    pub content: String,
}

fn normalize_directive_kind(raw: &str) -> Option<GraphitiEpisodeKind> {
    let normalized = raw.trim().to_lowercase().replace(['-', ' '], "_");

    match normalized.as_str() {
        "decision" => Some(GraphitiEpisodeKind::Decision),
        "lesson" | "lesson_learned" | "lessons" | "lessons_learned" | "remember" | "note" => {
            Some(GraphitiEpisodeKind::LessonLearned)
        }
        "preference" | "preferences" | "pref" | "prefs" => Some(GraphitiEpisodeKind::Preference),
        "procedure" | "steps" | "howto" | "how_to" => Some(GraphitiEpisodeKind::Procedure),
        "task" | "task_update" | "taskupdate" | "todo" => Some(GraphitiEpisodeKind::TaskUpdate),
        "terminology" | "term" | "naming" => Some(GraphitiEpisodeKind::Terminology),
        _ => None,
    }
}

fn normalize_directive_scope(raw: &str) -> Option<GraphitiScope> {
    match raw.trim().to_lowercase().as_str() {
        "workspace" => Some(GraphitiScope::Workspace),
        "global" => Some(GraphitiScope::Global),
        _ => None,
    }
}

fn parse_directive_header(header: &str) -> Option<(GraphitiEpisodeKind, Option<GraphitiScope>)> {
    let header = header.trim();
    if header.is_empty() {
        return None;
    }

    let (kind_part, scope_part) = if let Some(open) = header.find('(') {
        let close = header.rfind(')')?;
        if close <= open {
            return None;
        }
        let kind_part = header[..open].trim();
        let scope_part = header[open + 1..close].trim();
        (kind_part, Some(scope_part))
    } else {
        (header, None)
    };

    let kind = normalize_directive_kind(kind_part)?;
    let scope_override = scope_part.and_then(normalize_directive_scope);
    Some((kind, scope_override))
}

fn infer_scope(kind: GraphitiEpisodeKind, content: &str) -> GraphitiScope {
    // Project-scoped kinds default to workspace unless explicitly overridden.
    if matches!(
        kind,
        GraphitiEpisodeKind::Decision
            | GraphitiEpisodeKind::Procedure
            | GraphitiEpisodeKind::TaskUpdate
    ) {
        return GraphitiScope::Workspace;
    }

    let normalized = content.trim().to_lowercase();
    let user_cue = normalized.contains("i prefer")
        || normalized.contains("my preference")
        || normalized.contains("in general")
        || normalized.contains("across projects")
        || normalized.contains("across repos")
        || normalized.contains("across workspaces");

    if user_cue {
        return GraphitiScope::Global;
    }

    // Prefer the least persistent scope when ambiguous.
    GraphitiScope::Workspace
}

pub fn looks_like_secret_for_auto_promotion(content: &str) -> bool {
    let normalized = content.to_lowercase();

    // Keyword-based guard (intentionally conservative).
    if normalized.contains("password")
        || normalized.contains("passwd")
        || normalized.contains("pwd")
    {
        return true;
    }
    if normalized.contains("token") || normalized.contains("secret") {
        return true;
    }
    if normalized.contains("api key") || normalized.contains("apikey") {
        return true;
    }

    // Stronger signature-based guard.
    if content.to_ascii_lowercase().contains("-----begin")
        && content.to_ascii_lowercase().contains("private key-----")
    {
        return true;
    }
    if normalized.contains("ghp_")
        || normalized.contains("gho_")
        || normalized.contains("github_pat_")
    {
        return true;
    }
    if normalized.contains("sk-") {
        return true;
    }
    if normalized.contains("akia") {
        return true;
    }

    false
}

pub fn parse_memory_directives(message: &str) -> Vec<GraphitiMemoryDirective> {
    let lines = message.lines().collect::<Vec<_>>();
    let mut out: Vec<GraphitiMemoryDirective> = Vec::new();

    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        let Some(colon) = trimmed.find(':') else {
            i += 1;
            continue;
        };

        let header = &trimmed[..colon];
        let rest = trimmed[colon + 1..].trim();
        let Some((kind, scope_override)) = parse_directive_header(header) else {
            i += 1;
            continue;
        };

        let mut content_lines: Vec<String> = Vec::new();
        if !rest.is_empty() {
            content_lines.push(rest.to_string());
        } else {
            let mut j = i + 1;
            while j < lines.len() {
                let next = lines[j];
                if next.trim().is_empty() {
                    break;
                }
                content_lines.push(next.to_string());
                j += 1;
            }
            i = j.saturating_sub(1);
        }

        let content = content_lines.join("\n").trim().to_string();
        if content.is_empty() {
            i += 1;
            continue;
        }

        let scope = scope_override.unwrap_or_else(|| infer_scope(kind, &content));
        out.push(GraphitiMemoryDirective {
            kind,
            scope,
            content,
        });

        i += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_single_line_directive_with_explicit_scope_override() {
        let directives = parse_memory_directives("preference (global): Keep diffs small.");
        assert_eq!(
            directives,
            vec![GraphitiMemoryDirective {
                kind: GraphitiEpisodeKind::Preference,
                scope: GraphitiScope::Global,
                content: "Keep diffs small.".to_string(),
            }]
        );
    }

    #[test]
    fn parses_multi_line_directive_and_defaults_to_workspace_when_ambiguous() {
        let directives =
            parse_memory_directives("terminology:\nfoo means bar\nbaz means qux\n\nnope");
        assert_eq!(
            directives,
            vec![GraphitiMemoryDirective {
                kind: GraphitiEpisodeKind::Terminology,
                scope: GraphitiScope::Workspace,
                content: "foo means bar\nbaz means qux".to_string(),
            }]
        );
    }

    #[test]
    fn detects_likely_secrets_for_auto_promotion() {
        assert_eq!(
            looks_like_secret_for_auto_promotion("my password is 123"),
            true
        );
        assert_eq!(
            looks_like_secret_for_auto_promotion("use pnpm in this repo"),
            false
        );
    }
}
