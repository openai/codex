use crate::git_info::resolve_root_git_project_for_trust;
use codex_protocol::protocol::SubAgentSource;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;

const CONFIG_DIR: &str = ".codex";
const AGENTS_DIR: &str = "agents";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubAgentDefinition {
    pub name: String,
    pub description: String,
    pub color: Option<String>,
    pub prompt: String,
    pub path: PathBuf,
    pub source: SubAgentSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubAgentInvocation {
    pub name: String,
    pub prompt: String,
}

#[derive(Debug)]
pub enum SubAgentInvocationError {
    InvalidName(String),
}

impl fmt::Display for SubAgentInvocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubAgentInvocationError::InvalidName(name) => {
                write!(f, "invalid subagent name: {name}")
            }
        }
    }
}

impl Error for SubAgentInvocationError {}

#[derive(Debug)]
pub enum SubAgentResolveError {
    NotFound {
        name: String,
        searched_roots: Vec<PathBuf>,
        codex_home_error: Option<String>,
    },
    Read {
        path: PathBuf,
        message: String,
    },
    Parse {
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for SubAgentResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubAgentResolveError::NotFound {
                name,
                searched_roots,
                codex_home_error,
            } => {
                write!(f, "subagent `{name}` not found")?;
                if !searched_roots.is_empty() {
                    write!(f, "; searched:")?;
                    for root in searched_roots {
                        write!(f, " {}", root.display())?;
                    }
                }
                if let Some(err) = codex_home_error {
                    write!(f, "; CODEX_HOME unavailable: {err}")?;
                }
                Ok(())
            }
            SubAgentResolveError::Read { path, message } => {
                write!(f, "failed to read {}: {message}", path.display())
            }
            SubAgentResolveError::Parse { path, message } => {
                write!(f, "failed to parse {}: {message}", path.display())
            }
        }
    }
}

impl Error for SubAgentResolveError {}

#[derive(Debug, Deserialize)]
struct SubAgentFrontmatter {
    description: String,
    #[serde(default)]
    color: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_yaml::Value>,
}

pub fn parse_subagent_invocation(
    text: &str,
) -> Result<Option<SubAgentInvocation>, SubAgentInvocationError> {
    if !text.starts_with('@') {
        return Ok(None);
    }

    let after_at = &text[1..];
    // Treat `@name` (no whitespace) as not-a-subagent invocation so editor/UI
    // mention pickers can still operate without triggering an error.
    let Some((raw_name, remainder)) = after_at.split_once(char::is_whitespace) else {
        return Ok(None);
    };

    if raw_name.is_empty()
        || raw_name == "."
        || raw_name == ".."
        || raw_name.contains('/')
        || raw_name.contains('\\')
        || !raw_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(SubAgentInvocationError::InvalidName(raw_name.to_string()));
    }

    let prompt = remainder.trim().to_string();
    if prompt.is_empty() {
        return Ok(None);
    }

    Ok(Some(SubAgentInvocation {
        name: raw_name.to_string(),
        prompt,
    }))
}

pub async fn resolve_subagent_definition(
    cwd: &Path,
    name: &str,
) -> Result<SubAgentDefinition, SubAgentResolveError> {
    let mut roots = Vec::<PathBuf>::new();

    if let Some(repo_root) = resolve_root_git_project_for_trust(cwd) {
        roots.push(repo_root.join(CONFIG_DIR).join(AGENTS_DIR));
    }

    let (home_root, codex_home_error) = match crate::config::find_codex_home() {
        Ok(home) => (Some(vec![home.join(AGENTS_DIR)]), None),
        Err(err) => (None, Some(err.to_string())),
    };
    if let Some(home_roots) = home_root {
        roots.extend(home_roots);
    }

    let file_name = format!("{name}.md");
    for root in &roots {
        let path = root.join(&file_name);
        let Ok(meta) = fs::metadata(&path).await else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let contents = fs::read_to_string(&path)
            .await
            .map_err(|e| SubAgentResolveError::Read {
                path: path.clone(),
                message: e.to_string(),
            })?;
        return parse_subagent_file(name, &path, &contents).map_err(|message| {
            SubAgentResolveError::Parse {
                path: path.clone(),
                message,
            }
        });
    }

    Err(SubAgentResolveError::NotFound {
        name: name.to_string(),
        searched_roots: roots,
        codex_home_error,
    })
}

fn parse_subagent_file(
    name: &str,
    path: &Path,
    contents: &str,
) -> Result<SubAgentDefinition, String> {
    let (frontmatter, body) = extract_frontmatter_and_body(contents)
        .ok_or_else(|| "missing YAML frontmatter delimited by ---".to_string())?;

    let parsed: SubAgentFrontmatter =
        serde_yaml::from_str(&frontmatter).map_err(|e| format!("invalid YAML: {e}"))?;

    if !parsed.extra.is_empty() {
        let keys = parsed
            .extra
            .keys()
            .cloned()
            .collect::<Vec<String>>()
            .join(", ");
        return Err(format!(
            "unsupported frontmatter keys: {keys} (currently supported: description, color)"
        ));
    }

    let description = sanitize_single_line(&parsed.description);
    if description.is_empty() {
        return Err("missing field `description`".to_string());
    }
    let color = parsed
        .color
        .map(|c| sanitize_single_line(&c))
        .filter(|c| !c.is_empty());

    let prompt = body.trim().to_string();
    if prompt.is_empty() {
        return Err("subagent prompt body is empty".to_string());
    }

    Ok(SubAgentDefinition {
        name: name.to_string(),
        description,
        color,
        prompt,
        path: path.to_path_buf(),
        source: SubAgentSource::Other(name.to_string()),
    })
}

fn sanitize_single_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_frontmatter_and_body(contents: &str) -> Option<(String, String)> {
    let mut segments = contents.split_inclusive('\n');
    let first_segment = segments.next()?;
    let first_line = first_segment.trim_end_matches(['\r', '\n']);
    if first_line.trim() != "---" {
        return None;
    }

    let mut frontmatter_lines: Vec<String> = Vec::new();
    let mut frontmatter_closed = false;
    let mut consumed = first_segment.len();

    for segment in segments {
        let line = segment.trim_end_matches(['\r', '\n']);
        if line.trim() == "---" {
            frontmatter_closed = true;
            consumed += segment.len();
            break;
        }
        frontmatter_lines.push(line.to_string());
        consumed += segment.len();
    }

    if frontmatter_lines.is_empty() || !frontmatter_closed {
        return None;
    }

    let frontmatter = frontmatter_lines.join("\n");
    let body = if consumed >= contents.len() {
        String::new()
    } else {
        contents[consumed..].to_string()
    };
    Some((frontmatter, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_invocation_none_when_not_prefixed() {
        let got = parse_subagent_invocation("hello").unwrap();
        assert_eq!(got, None);
    }

    #[test]
    fn parse_invocation_none_when_prompt_missing_without_space() {
        let got = parse_subagent_invocation("@helper").unwrap();
        assert_eq!(got, None);
    }

    #[test]
    fn parse_invocation_none_when_prompt_missing_with_space() {
        let got = parse_subagent_invocation("@helper ").unwrap();
        assert_eq!(got, None);
    }

    #[test]
    fn parse_invocation_rejects_path_like_name() {
        let err = parse_subagent_invocation("@../oops hi").unwrap_err();
        assert!(matches!(err, SubAgentInvocationError::InvalidName(_)));
    }

    #[test]
    fn parse_invocation_ok() {
        let got = parse_subagent_invocation("@helper  do the thing").unwrap();
        assert_eq!(
            got,
            Some(SubAgentInvocation {
                name: "helper".to_string(),
                prompt: "do the thing".to_string(),
            })
        );
    }

    #[test]
    fn parse_subagent_file_requires_frontmatter() {
        let err = parse_subagent_file("x", Path::new("x.md"), "nope").unwrap_err();
        assert!(err.contains("frontmatter"));
    }

    #[test]
    fn parse_subagent_file_rejects_unknown_frontmatter_keys() {
        let content = r#"---
description: hi
mode: system
---
prompt"#;
        let err = parse_subagent_file("x", Path::new("x.md"), content).unwrap_err();
        assert!(err.contains("unsupported frontmatter keys"));
    }

    #[test]
    fn parse_subagent_file_accepts_color() {
        let content = r#"---
description: hi
color: cyan
---
prompt"#;
        let def = parse_subagent_file("x", Path::new("x.md"), content).unwrap();
        assert_eq!(def.color, Some("cyan".to_string()));
    }

    #[test]
    fn parse_subagent_file_ok() {
        let content = r#"---
description:  Hello   world
---
You are helpful."#;
        let def = parse_subagent_file("helper", Path::new("helper.md"), content).unwrap();
        assert_eq!(def.name, "helper");
        assert_eq!(def.description, "Hello world");
        assert_eq!(def.prompt, "You are helpful.");
        assert_eq!(def.source, SubAgentSource::Other("helper".to_string()));
    }
}
