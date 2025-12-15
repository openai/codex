use codex_core::git_info::resolve_root_git_project_for_trust;
use ratatui::style::Color;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

const CONFIG_DIR: &str = ".codex";
const AGENTS_DIR: &str = "agents";

#[derive(Debug, Clone)]
pub(crate) struct SubAgentMatch {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) color: Option<Color>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubAgentFrontmatter {
    description: String,
    #[serde(default)]
    color: Option<String>,
}

pub(crate) fn search_subagents(cwd: &Path, codex_home: &Path, query: &str) -> Vec<SubAgentMatch> {
    let query_lower = query.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut seen = HashSet::<String>::new();

    for root in subagent_roots(cwd, codex_home) {
        let Ok(read_dir) = fs::read_dir(&root) else {
            continue;
        };

        let mut entries = read_dir
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                let is_md = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));
                if !is_md {
                    return None;
                }
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::to_string)?;
                Some((name, path))
            })
            .collect::<Vec<(String, PathBuf)>>();
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, path) in entries {
            if !seen.insert(name.clone()) {
                continue;
            }
            if !query_lower.is_empty() && !name.to_ascii_lowercase().starts_with(&query_lower) {
                continue;
            }

            let Ok(contents) = fs::read_to_string(&path) else {
                tracing::trace!("failed to read subagent definition: {}", path.display());
                continue;
            };
            let Some((frontmatter, _body)) = extract_frontmatter_and_body(&contents) else {
                tracing::trace!(
                    "subagent definition missing frontmatter: {}",
                    path.display()
                );
                continue;
            };
            let Ok(parsed) = serde_yaml::from_str::<SubAgentFrontmatter>(&frontmatter) else {
                tracing::debug!("failed to parse subagent frontmatter: {}", path.display());
                continue;
            };

            let description = sanitize_single_line(&parsed.description);
            if description.is_empty() {
                continue;
            }
            let color = parsed.color.as_deref().map(str::trim).and_then(parse_color);

            out.push(SubAgentMatch {
                name,
                description,
                color,
            });
        }
    }

    out
}

fn subagent_roots(cwd: &Path, codex_home: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(repo_root) = resolve_root_git_project_for_trust(cwd) {
        roots.push(repo_root.join(CONFIG_DIR).join(AGENTS_DIR));
    }
    roots.push(codex_home.join(AGENTS_DIR));

    roots
}

fn parse_color(raw: &str) -> Option<Color> {
    match raw.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        _ => None,
    }
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

    #[test]
    fn parse_color_maps_known_names() {
        assert_eq!(parse_color("cyan"), Some(Color::Cyan));
        assert_eq!(parse_color("Grey"), Some(Color::Gray));
        assert_eq!(parse_color("nope"), None);
    }
}
