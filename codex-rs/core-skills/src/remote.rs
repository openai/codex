use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use codex_login::CodexAuth;

const REMOTE_SKILLS_API_TIMEOUT: Duration = Duration::from_secs(30);

// Low-level client for the remote skill API. This is intentionally kept around for
// future wiring, but it is not used yet by any active product surface.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteSkillScope {
    WorkspaceShared,
    AllShared,
    Personal,
    Example,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteSkillProductSurface {
    Chatgpt,
    Codex,
    Api,
    Atlas,
}

fn as_query_scope(scope: RemoteSkillScope) -> Option<&'static str> {
    match scope {
        RemoteSkillScope::WorkspaceShared => Some("workspace-shared"),
        RemoteSkillScope::AllShared => Some("all-shared"),
        RemoteSkillScope::Personal => Some("personal"),
        RemoteSkillScope::Example => Some("example"),
    }
}

fn as_query_product_surface(product_surface: RemoteSkillProductSurface) -> &'static str {
    match product_surface {
        RemoteSkillProductSurface::Chatgpt => "chatgpt",
        RemoteSkillProductSurface::Codex => "codex",
        RemoteSkillProductSurface::Api => "api",
        RemoteSkillProductSurface::Atlas => "atlas",
    }
}

fn ensure_codex_backend_auth(auth: Option<&CodexAuth>) -> Result<&CodexAuth> {
    let Some(auth) = auth else {
        anyhow::bail!("chatgpt authentication required for remote skill scopes");
    };
    if !auth.uses_codex_backend() {
        anyhow::bail!(
            "chatgpt authentication required for remote skill scopes; api key auth is not supported"
        );
    }
    Ok(auth)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSkillDownloadResult {
    pub id: String,
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RemoteSkillsResponse {
    #[serde(rename = "hazelnuts")]
    skills: Vec<RemoteSkill>,
}

#[derive(Debug, Deserialize)]
struct RemoteSkill {
    id: String,
    name: String,
    description: String,
}

fn safe_join(base: &Path, name: &str) -> Result<PathBuf> {
    let path = Path::new(name);
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => {
                anyhow::bail!("Invalid file path in remote skill payload: {name}");
            }
        }
    }
    Ok(base.join(path))
}

fn is_zip_payload(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08")
}

fn extract_zip_to_dir(
    bytes: Vec<u8>,
    output_dir: &Path,
    prefix_candidates: &[String],
) -> Result<()> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).context("Failed to open zip archive")?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("Failed to read zip entry")?;
        if file.is_dir() {
            continue;
        }
        let raw_name = file.name().to_string();
        let normalized = normalize_zip_name(&raw_name, prefix_candidates);
        let Some(normalized) = normalized else {
            continue;
        };
        let file_path = safe_join(output_dir, &normalized)?;
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent dir for {normalized}"))?;
        }
        let mut out = std::fs::File::create(&file_path)
            .with_context(|| format!("Failed to create file {normalized}"))?;
        std::io::copy(&mut file, &mut out)
            .with_context(|| format!("Failed to write skill file {normalized}"))?;
    }
    Ok(())
}

fn normalize_zip_name(name: &str, prefix_candidates: &[String]) -> Option<String> {
    let mut trimmed = name.trim_start_matches("./");
    for prefix in prefix_candidates {
        if prefix.is_empty() {
            continue;
        }
        let prefix = format!("{prefix}/");
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            trimmed = rest;
            break;
        }
    }
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
