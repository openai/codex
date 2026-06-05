//! Codex App directives embedded in assistant markdown.

use std::collections::HashSet;
use std::path::Path;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum GitActionDirective {
    Stage {
        cwd: String,
    },
    Commit {
        cwd: String,
    },
    CreateBranch {
        cwd: String,
        branch: String,
    },
    Push {
        cwd: String,
        branch: String,
    },
    CreatePr {
        cwd: String,
        branch: String,
        url: Option<String>,
        is_draft: bool,
    },
}

impl GitActionDirective {
    pub(crate) fn created_branch_cwd(&self) -> Option<&str> {
        match self {
            Self::CreateBranch { cwd, .. } => Some(cwd),
            _ => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ParsedAssistantMarkdown {
    pub(crate) visible_markdown: String,
    pub(crate) git_actions: Vec<GitActionDirective>,
}

impl ParsedAssistantMarkdown {
    pub(crate) fn last_created_branch_cwd(&self) -> Option<&str> {
        self.git_actions
            .iter()
            .rev()
            .find_map(GitActionDirective::created_branch_cwd)
    }
}

pub(crate) fn parse_assistant_markdown(markdown: &str, cwd: &Path) -> ParsedAssistantMarkdown {
    let mut git_actions = Vec::new();
    let mut seen = HashSet::new();
    let mut visible_lines = Vec::new();

    for line in markdown.lines() {
        let rewritten = rewrite_code_comment_line(line, cwd);
        let line = rewritten.as_deref().unwrap_or(line);
        let (visible_line, line_actions) = strip_line_directives(line);
        for action in line_actions {
            if seen.insert(action.clone()) {
                git_actions.push(action);
            }
        }
        visible_lines.push(visible_line.trim_end().to_string());
    }

    while visible_lines
        .last()
        .is_some_and(std::string::String::is_empty)
    {
        visible_lines.pop();
    }

    ParsedAssistantMarkdown {
        visible_markdown: visible_lines.join("\n"),
        git_actions,
    }
}

fn rewrite_code_comment_line(line: &str, cwd: &Path) -> Option<String> {
    let content = line.trim_start_matches([' ', '\t']);
    let indent = &line[..line.len() - content.len()];
    let marker_length = content.bytes().take_while(|byte| *byte == b':').count();
    if !(1..=3).contains(&marker_length) {
        return None;
    }

    let directive = content[marker_length..].strip_prefix("code-comment{")?;
    let (attributes, suffix) = directive.rsplit_once('}')?;
    let title = code_comment_attribute(attributes, "title")?;
    let body = code_comment_attribute(attributes, "body")?;
    let file = code_comment_attribute(attributes, "file")?;
    let title = title.trim();
    let body = body.trim();
    let file = file.trim();
    (!title.is_empty() && !body.is_empty() && !file.is_empty()).then_some(())?;

    let start = directive_integer(attributes, "start").unwrap_or(1).max(1);
    let end = directive_integer(attributes, "end")
        .unwrap_or(start)
        .max(start);
    let title = if title_has_priority(title) {
        title.to_string()
    } else if let Some(priority @ 0..=3) = directive_integer(attributes, "priority") {
        format!("[P{priority}] {title}")
    } else {
        title.to_string()
    };
    let file_path = Path::new(file);
    let file = file_path
        .strip_prefix(cwd)
        .unwrap_or(file_path)
        .to_string_lossy()
        .replace('\\', "/");
    let location = if start == end {
        format!("{file}:{start}")
    } else {
        format!("{file}:{start}-{end}")
    };

    Some(format!(
        "{indent}- {title} — {location}\n{indent}  {body}{suffix}"
    ))
}

fn strip_line_directives(line: &str) -> (String, Vec<GitActionDirective>) {
    let mut visible = String::new();
    let mut actions = Vec::new();
    let mut remaining = line;

    while let Some(start) = remaining.find("::git-") {
        visible.push_str(&remaining[..start]);
        let directive = &remaining[start + 2..];
        let Some(open_brace) = directive.find('{') else {
            visible.push_str(&remaining[start..]);
            return (visible, actions);
        };
        let Some(close_brace) = directive[open_brace + 1..].find('}') else {
            visible.push_str(&remaining[start..]);
            return (visible, actions);
        };
        let close_brace = open_brace + 1 + close_brace;
        let name = &directive[..open_brace];
        let attributes = &directive[open_brace + 1..close_brace];
        if let Some(action) = parse_git_action(name, attributes) {
            actions.push(action);
        }
        remaining = &directive[close_brace + 1..];
    }
    visible.push_str(remaining);
    (visible, actions)
}

fn directive_integer(attributes: &str, name: &str) -> Option<i64> {
    code_comment_attribute(attributes, name)?
        .trim()
        .trim_start_matches(['P', 'p'])
        .parse()
        .ok()
}

fn title_has_priority(title: &str) -> bool {
    let bytes = title.trim_start().as_bytes();
    bytes.len() >= 4
        && bytes[0] == b'['
        && matches!(bytes[1], b'P' | b'p')
        && bytes[2].is_ascii_digit()
        && bytes[3] == b']'
}

fn code_comment_attribute(attributes: &str, name: &str) -> Option<String> {
    let marker = format!("{name}=");
    let start = attributes.match_indices(&marker).find_map(|(index, _)| {
        (index == 0 || attributes.as_bytes()[index - 1].is_ascii_whitespace()).then_some(index)
    })?;
    let value = &attributes[start + marker.len()..];
    if let Some(value) = value.strip_prefix('"') {
        parse_quoted_value(value).map(|(value, _)| value)
    } else {
        Some(value.split_whitespace().next()?.to_string())
    }
}

fn parse_git_action(name: &str, attributes: &str) -> Option<GitActionDirective> {
    let attrs = parse_attributes(attributes)?;
    let cwd = attrs.get("cwd")?.clone();
    match name {
        "git-stage" => Some(GitActionDirective::Stage { cwd }),
        "git-commit" => Some(GitActionDirective::Commit { cwd }),
        "git-create-branch" => Some(GitActionDirective::CreateBranch {
            cwd,
            branch: attrs.get("branch")?.clone(),
        }),
        "git-push" => Some(GitActionDirective::Push {
            cwd,
            branch: attrs.get("branch")?.clone(),
        }),
        "git-create-pr" => Some(GitActionDirective::CreatePr {
            cwd,
            branch: attrs.get("branch")?.clone(),
            url: attrs.get("url").cloned(),
            is_draft: attrs.get("isDraft").is_some_and(|value| value == "true"),
        }),
        _ => None,
    }
}

fn parse_attributes(input: &str) -> Option<std::collections::HashMap<String, String>> {
    let mut attrs = std::collections::HashMap::new();
    let mut rest = input.trim();
    while !rest.is_empty() {
        let eq = rest.find('=')?;
        let key = rest[..eq].trim();
        if key.is_empty() {
            return None;
        }
        rest = rest[eq + 1..].trim_start();
        let (value, next) = if let Some(quoted) = rest.strip_prefix('"') {
            let end = quoted.find('"')?;
            (quoted[..end].to_string(), &quoted[end + 1..])
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            (rest[..end].to_string(), &rest[end..])
        };
        attrs.insert(key.to_string(), value);
        rest = next.trim_start();
    }
    Some(attrs)
}

fn parse_quoted_value(input: &str) -> Option<(String, &str)> {
    let mut value = String::new();
    let mut characters = input.char_indices().peekable();

    while let Some((index, character)) = characters.next() {
        if character == '"' {
            return Some((value, &input[index + 1..]));
        }
        match character {
            '\\' if characters.peek().is_some_and(|(_, next)| *next == '"') => {
                value.push('"');
                characters.next();
            }
            _ => value.push(character),
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn strips_and_parses_git_action_directives() {
        let parsed = parse_assistant_markdown(
            "Done\n\n::git-stage{cwd=\"/repo\"} ::git-push{cwd=\"/repo\" branch=\"feat/x\"} ::git-stage{cwd=\"C:\\repo\\\"}",
            Path::new("/repo"),
        );

        assert_eq!(parsed.visible_markdown, "Done");
        assert_eq!(
            parsed.git_actions,
            vec![
                GitActionDirective::Stage {
                    cwd: "/repo".to_string(),
                },
                GitActionDirective::Push {
                    cwd: "/repo".to_string(),
                    branch: "feat/x".to_string(),
                },
                GitActionDirective::Stage {
                    cwd: "C:\\repo\\".to_string(),
                },
            ]
        );
    }

    #[test]
    fn hides_malformed_directives_without_materializing_rows() {
        let parsed = parse_assistant_markdown("Done ::git-push{cwd=\"/repo\"}", Path::new("/repo"));

        assert_eq!(parsed.visible_markdown, "Done");
        assert!(parsed.git_actions.is_empty());
    }

    #[test]
    fn renders_code_comment_directives_as_markdown() {
        let parsed = parse_assistant_markdown(
            "Found two issues.\n\n::code-comment{title=\"Preserve state\" body=\"This changes role=\\\"tab\\\" and the `${value}` lookup.\" file=\"/repo/src/app.ts\" start=10 end=12 priority=\"P2\"}\n\n:::code-comment{title=\"[P1] Clamp the range\" body=\"The line range should match the App.\" file=\"codex/src/range.ts\" start=8 end=2 priority=3}",
            Path::new("/repo"),
        );

        insta::assert_snapshot!("code_comment_directive_fallback", parsed.visible_markdown);
        assert!(parsed.git_actions.is_empty());
    }

    #[test]
    fn preserves_non_directive_and_malformed_code_comment_text() {
        let markdown = "Mention `::code-comment{title=\"Example\"}` inline.\n::code-comment{title=\"Missing body\" file=\"/repo/src/app.ts\"}";
        let parsed = parse_assistant_markdown(markdown, Path::new("/repo"));

        assert_eq!(parsed.visible_markdown, markdown);
    }

    #[test]
    fn last_created_branch_cwd_uses_the_last_matching_directive() {
        let parsed = parse_assistant_markdown(
            "::git-create-branch{cwd=\"/first\" branch=\"first\"}\n::git-push{cwd=\"/repo\" branch=\"first\"}\n::git-create-branch{cwd=\"/second\" branch=\"second\"}",
            Path::new("/repo"),
        );

        assert_eq!(parsed.last_created_branch_cwd(), Some("/second"));
    }
}
