//! Codex App git action directives embedded in assistant markdown.

use std::collections::HashSet;

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

pub(crate) fn parse_assistant_markdown(markdown: &str) -> ParsedAssistantMarkdown {
    let mut git_actions = Vec::new();
    let mut seen = HashSet::new();
    let mut visible_lines = Vec::new();

    for line in markdown.lines() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_and_parses_git_action_directives() {
        let parsed = parse_assistant_markdown(
            "Done\n\n::git-stage{cwd=\"/repo\"} ::git-push{cwd=\"/repo\" branch=\"feat/x\"}",
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
            ]
        );
    }

    #[test]
    fn hides_malformed_directives_without_materializing_rows() {
        let parsed = parse_assistant_markdown("Done ::git-push{cwd=\"/repo\"}");

        assert_eq!(parsed.visible_markdown, "Done");
        assert!(parsed.git_actions.is_empty());
    }

    #[test]
    fn last_created_branch_cwd_uses_the_last_matching_directive() {
        let parsed = parse_assistant_markdown(
            "::git-create-branch{cwd=\"/first\" branch=\"first\"}\n::git-push{cwd=\"/repo\" branch=\"first\"}\n::git-create-branch{cwd=\"/second\" branch=\"second\"}",
        );

        assert_eq!(parsed.last_created_branch_cwd(), Some("/second"));
    }
}
