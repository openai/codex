use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use crate::auth::CodexAuth;
use crate::config::Config;
use crate::default_client::build_reqwest_client;
use codex_protocol::protocol::RemoteSkillHazelnutScope;
use codex_protocol::protocol::RemoteSkillProductSurface;

const REMOTE_SKILLS_API_TIMEOUT: Duration = Duration::from_secs(30);

fn as_query_hazelnut_scope(scope: RemoteSkillHazelnutScope) -> Option<&'static str> {
    match scope {
        RemoteSkillHazelnutScope::WorkspaceShared => Some("workspace-shared"),
        RemoteSkillHazelnutScope::AllShared => Some("all-shared"),
        RemoteSkillHazelnutScope::Personal => Some("personal"),
        RemoteSkillHazelnutScope::Example => Some("example"),
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

fn ensure_chatgpt_auth(auth: Option<&CodexAuth>) -> Result<&CodexAuth> {
    let Some(auth) = auth else {
        anyhow::bail!("chatgpt authentication required for hazelnut scopes");
    };
    if !auth.is_chatgpt_auth() {
        anyhow::bail!(
            "chatgpt authentication required for hazelnut scopes; api key auth is not supported"
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
    hazelnuts: Vec<RemoteSkill>,
}

#[derive(Debug, Deserialize)]
struct RemoteSkill {
    id: String,
    name: String,
    description: String,
}

pub async fn list_remote_skills(
    config: &Config,
    auth: Option<&CodexAuth>,
    hazelnut_scope: RemoteSkillHazelnutScope,
    product_surface: RemoteSkillProductSurface,
    enabled: Option<bool>,
) -> Result<Vec<RemoteSkillSummary>> {
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let auth = ensure_chatgpt_auth(auth)?;

    let url = format!("{base_url}/hazelnuts");
    let product_surface = as_query_product_surface(product_surface);
    let mut query_params = vec![("product_surface", product_surface)];
    if let Some(scope) = as_query_hazelnut_scope(hazelnut_scope) {
        query_params.push(("scope", scope));
    }
    if let Some(enabled) = enabled {
        let enabled = if enabled { "true" } else { "false" };
        query_params.push(("enabled", enabled));
    }

    let client = build_reqwest_client();
    let mut request = client
        .get(&url)
        .timeout(REMOTE_SKILLS_API_TIMEOUT)
        .query(&query_params);
    let token = auth
        .get_token()
        .context("Failed to read auth token for remote skills")?;
    request = request.bearer_auth(token);
    if let Some(account_id) = auth.get_account_id() {
        request = request.header("chatgpt-account-id", account_id);
    }
    let response = request
        .send()
        .await
        .with_context(|| format!("Failed to send request to {url}"))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("Request failed with status {status} from {url}: {body}");
    }

    let parsed: RemoteSkillsResponse =
        serde_json::from_str(&body).context("Failed to parse skills response")?;

    Ok(parsed
        .hazelnuts
        .into_iter()
        .map(|skill| RemoteSkillSummary {
            id: skill.id,
            name: skill.name,
            description: skill.description,
        })
        .collect())
}

pub async fn export_remote_skill(
    config: &Config,
    auth: Option<&CodexAuth>,
    hazelnut_id: &str,
) -> Result<RemoteSkillDownloadResult> {
    let auth = ensure_chatgpt_auth(auth)?;

    let client = build_reqwest_client();
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/hazelnuts/{hazelnut_id}/export");
    let mut request = client.get(&url).timeout(REMOTE_SKILLS_API_TIMEOUT);

    let token = auth
        .get_token()
        .context("Failed to read auth token for remote skills")?;
    request = request.bearer_auth(token);
    if let Some(account_id) = auth.get_account_id() {
        request = request.header("chatgpt-account-id", account_id);
    }

    let response = request
        .send()
        .await
        .with_context(|| format!("Failed to send download request to {url}"))?;

    let status = response.status();
    let body = response.bytes().await.context("Failed to read download")?;
    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body);
        anyhow::bail!("Download failed with status {status} from {url}: {body_text}");
    }

    if !is_zip_payload(&body) {
        anyhow::bail!("Downloaded remote skill payload is not a zip archive");
    }

    let output_dir = config.codex_home.join("skills").join(hazelnut_id);
    tokio::fs::create_dir_all(&output_dir)
        .await
        .context("Failed to create downloaded skills directory")?;

    let zip_bytes = body.to_vec();
    let output_dir_clone = output_dir.clone();
    let prefix_candidates = vec![hazelnut_id.to_string()];
    tokio::task::spawn_blocking(move || {
        extract_zip_to_dir(zip_bytes, &output_dir_clone, &prefix_candidates)
    })
    .await
    .context("Zip extraction task failed")??;

    Ok(RemoteSkillDownloadResult {
        id: hazelnut_id.to_string(),
        path: output_dir,
    })
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
