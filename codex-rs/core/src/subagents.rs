use crate::config::AgentsSource;
use crate::git_info::resolve_root_git_project_for_trust;
use codex_protocol::protocol::SubAgentSource;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::HashSet;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubAgentScope {
    Repo,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubAgentSummary {
    pub name: String,
    pub description: String,
    pub color: Option<String>,
    pub path: PathBuf,
    pub scope: SubAgentScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubAgentError {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubAgentListOutcome {
    pub agents: Vec<SubAgentSummary>,
    pub errors: Vec<SubAgentError>,
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
    let (codex_home, codex_home_error) = match crate::config::find_codex_home() {
        Ok(home) => (Some(home), None),
        Err(err) => (None, Some(err.to_string())),
    };

    let mut roots = Vec::<(PathBuf, SubAgentScope)>::new();
    if let Some(repo_root) = resolve_root_git_project_for_trust(cwd) {
        roots.push((
            repo_root.join(CONFIG_DIR).join(AGENTS_DIR),
            SubAgentScope::Repo,
        ));
    }
    if let Some(codex_home) = codex_home.as_ref() {
        roots.push((codex_home.join(AGENTS_DIR), SubAgentScope::User));
    }

    resolve_subagent_definition_from_roots(cwd, name, roots, codex_home_error).await
}

pub async fn resolve_subagent_definition_with_sources(
    cwd: &Path,
    codex_home: &Path,
    sources: &[AgentsSource],
    name: &str,
) -> Result<SubAgentDefinition, SubAgentResolveError> {
    let roots = subagent_roots_for_sources(cwd, codex_home, sources);
    resolve_subagent_definition_from_roots(cwd, name, roots, None).await
}

async fn resolve_subagent_definition_from_roots(
    _cwd: &Path,
    name: &str,
    roots: Vec<(PathBuf, SubAgentScope)>,
    codex_home_error: Option<String>,
) -> Result<SubAgentDefinition, SubAgentResolveError> {
    let searched_roots = roots.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>();

    let file_name = format!("{name}.md");
    for (root, _scope) in &roots {
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
        searched_roots,
        codex_home_error,
    })
}

pub async fn list_subagents(
    cwd: &Path,
    codex_home: &Path,
    sources: &[AgentsSource],
) -> SubAgentListOutcome {
    let mut outcome = SubAgentListOutcome::default();
    let roots = subagent_roots_for_sources(cwd, codex_home, sources);

    let mut seen: HashSet<String> = HashSet::new();
    for (root, scope) in roots {
        let names = match list_markdown_stems(&root).await {
            Ok(names) => names,
            Err(message) => {
                outcome.errors.push(SubAgentError {
                    path: root.clone(),
                    message,
                });
                continue;
            }
        };

        for name in names {
            if !seen.insert(name.clone()) {
                continue;
            }
            if !is_valid_subagent_name(&name) {
                outcome.errors.push(SubAgentError {
                    path: root.join(format!("{name}.md")),
                    message: format!("invalid subagent name: {name}"),
                });
                continue;
            }

            let path = root.join(format!("{name}.md"));
            let contents = match fs::read_to_string(&path).await {
                Ok(contents) => contents,
                Err(err) => {
                    outcome.errors.push(SubAgentError {
                        path: path.clone(),
                        message: err.to_string(),
                    });
                    continue;
                }
            };

            let summary = match parse_subagent_summary_file(&name, &path, &contents, scope) {
                Ok(summary) => summary,
                Err(message) => {
                    outcome.errors.push(SubAgentError {
                        path: path.clone(),
                        message,
                    });
                    continue;
                }
            };
            outcome.agents.push(summary);
        }
    }

    outcome
        .agents
        .sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));

    outcome
}

fn subagent_roots_for_sources(
    cwd: &Path,
    codex_home: &Path,
    sources: &[AgentsSource],
) -> Vec<(PathBuf, SubAgentScope)> {
    let mut roots = Vec::<(PathBuf, SubAgentScope)>::new();

    for source in sources {
        match source {
            AgentsSource::Repo => {
                if let Some(repo_root) = resolve_root_git_project_for_trust(cwd) {
                    roots.push((
                        repo_root.join(CONFIG_DIR).join(AGENTS_DIR),
                        SubAgentScope::Repo,
                    ));
                }
            }
            AgentsSource::User => {
                roots.push((codex_home.join(AGENTS_DIR), SubAgentScope::User));
            }
        }
    }

    roots
}

async fn list_markdown_stems(dir: &Path) -> Result<Vec<String>, String> {
    let mut rd = match fs::read_dir(dir).await {
        Ok(rd) => rd,
        Err(err) => {
            if matches!(
                err.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) {
                return Ok(Vec::new());
            }
            return Err(err.to_string());
        }
    };
    let mut out = Vec::new();
    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();
        let meta = match entry.metadata().await {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if !meta.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if !ext.eq_ignore_ascii_case("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let name = stem.trim();
        if name.is_empty() {
            continue;
        }
        out.push(name.to_string());
    }
    out.sort();
    Ok(out)
}

fn is_valid_subagent_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
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

fn parse_subagent_summary_file(
    name: &str,
    path: &Path,
    contents: &str,
    scope: SubAgentScope,
) -> Result<SubAgentSummary, String> {
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

    let prompt = body.trim();
    if prompt.is_empty() {
        return Err("subagent prompt body is empty".to_string());
    }

    Ok(SubAgentSummary {
        name: name.to_string(),
        description,
        color,
        path: path.to_path_buf(),
        scope,
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
