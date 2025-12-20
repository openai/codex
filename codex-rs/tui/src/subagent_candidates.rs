use std::collections::HashSet;
use std::path::Path;

use codex_core::git_info::resolve_root_git_project_for_trust;
use codex_core::subagents::SubAgentResolveError;
use tokio::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SubAgentCandidate {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) disabled_reason: Option<String>,
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

async fn list_agent_names_in_dir(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut entries = match fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(_) => return out,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Ok(meta) = fs::metadata(&path).await else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let is_md = path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));
        if !is_md {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        out.push(stem.to_string());
    }

    out.sort();
    out
}

pub(crate) async fn discover_subagent_candidates(
    cwd: &Path,
    codex_home: &Path,
) -> Vec<SubAgentCandidate> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<SubAgentCandidate> = Vec::new();

    let repo_root = resolve_root_git_project_for_trust(cwd);
    let mut roots = Vec::new();
    if let Some(repo_root) = repo_root {
        roots.push(repo_root.join(".codex").join("agents"));
    }
    roots.push(codex_home.join("agents"));

    for root in roots {
        let names = list_agent_names_in_dir(&root).await;
        for name in names {
            if !seen.insert(name.clone()) {
                continue;
            }

            if !is_valid_subagent_name(&name) {
                out.push(SubAgentCandidate {
                    name,
                    description: None,
                    disabled_reason: Some("invalid subagent name (file stem)".to_string()),
                });
                continue;
            }

            match codex_core::subagents::resolve_subagent_definition(cwd, &name).await {
                Ok(def) => out.push(SubAgentCandidate {
                    name: def.name,
                    description: Some(def.description),
                    disabled_reason: None,
                }),
                Err(SubAgentResolveError::NotFound { .. }) => {}
                Err(err) => out.push(SubAgentCandidate {
                    name,
                    description: None,
                    disabled_reason: Some(err.to_string()),
                }),
            }
        }
    }

    out
}
