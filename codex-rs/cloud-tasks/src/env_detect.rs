use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use tracing::info;
use tracing::warn;

#[derive(Debug, Clone, serde::Deserialize)]
struct CodeEnvironment {
    id: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    is_pinned: Option<bool>,
    #[serde(default)]
    task_count: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AutodetectSelection {
    pub id: String,
    pub label: Option<String>,
}

pub async fn autodetect_environment_id(
    base_url: &str,
    headers: &HeaderMap,
    desired_label: Option<String>,
) -> anyhow::Result<AutodetectSelection> {
    // 1) Try repo-specific environments based on local git origins (GitHub only, like VSCode)
    let origins = get_git_origins();
    crate::append_error_log(format!("env: git origins: {origins:?}"));
    let mut by_repo_envs: Vec<CodeEnvironment> = Vec::new();
    for origin in &origins {
        if let Some((owner, repo)) = parse_owner_repo(origin) {
            let url = if base_url.contains("/backend-api") {
                format!(
                    "{}/wham/environments/by-repo/{}/{}/{}",
                    base_url, "github", owner, repo
                )
            } else {
                format!(
                    "{}/api/codex/environments/by-repo/{}/{}/{}",
                    base_url, "github", owner, repo
                )
            };
            crate::append_error_log(format!("env: GET {url}"));
            match get_json::<Vec<CodeEnvironment>>(&url, headers).await {
                Ok(mut list) => {
                    crate::append_error_log(format!(
                        "env: by-repo returned {} env(s) for {owner}/{repo}",
                        list.len(),
                    ));
                    by_repo_envs.append(&mut list);
                }
                Err(e) => crate::append_error_log(format!(
                    "env: by-repo fetch failed for {owner}/{repo}: {e}"
                )),
            }
        }
    }
    if let Some(env) = pick_environment_row(&by_repo_envs, desired_label.as_deref()) {
        return Ok(AutodetectSelection {
            id: env.id.clone(),
            label: env.label.as_deref().map(str::to_owned),
        });
    }

    // 2) Fallback to the full list
    let list_url = if base_url.contains("/backend-api") {
        format!("{base_url}/wham/environments")
    } else {
        format!("{base_url}/api/codex/environments")
    };
    crate::append_error_log(format!("env: GET {list_url}"));
    // Fetch and log the full environments JSON for debugging
    let http = reqwest::Client::builder().build()?;
    let res = http.get(&list_url).headers(headers.clone()).send().await?;
    let status = res.status();
    let ct = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = res.text().await.unwrap_or_default();
    crate::append_error_log(format!("env: status={status} content-type={ct}"));
    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(v) => {
            let pretty = serde_json::to_string_pretty(&v).unwrap_or(body.clone());
            crate::append_error_log(format!("env: /environments JSON (pretty):\n{pretty}"));
        }
        Err(_) => crate::append_error_log(format!("env: /environments (raw):\n{body}")),
    }
    if !status.is_success() {
        anyhow::bail!("GET {list_url} failed: {status}; content-type={ct}; body={body}");
    }
    let all_envs: Vec<CodeEnvironment> = serde_json::from_str(&body).map_err(|e| {
        anyhow::anyhow!("Decode error for {list_url}: {e}; content-type={ct}; body={body}")
    })?;
    if let Some(env) = pick_environment_row(&all_envs, desired_label.as_deref()) {
        return Ok(AutodetectSelection {
            id: env.id.clone(),
            label: env.label.as_deref().map(str::to_owned),
        });
    }
    anyhow::bail!("no environments available")
}

fn pick_environment_row(
    envs: &[CodeEnvironment],
    desired_label: Option<&str>,
) -> Option<CodeEnvironment> {
    if envs.is_empty() {
        return None;
    }
    if let Some(label) = desired_label {
        let lc = label.to_lowercase();
        if let Some(e) = envs
            .iter()
            .find(|e| e.label.as_deref().unwrap_or("").to_lowercase() == lc)
        {
            crate::append_error_log(format!("env: matched by label: {label} -> {}", e.id));
            return Some(e.clone());
        }
    }
    if envs.len() == 1 {
        crate::append_error_log("env: single environment available; selecting it");
        return Some(envs[0].clone());
    }
    if let Some(e) = envs.iter().find(|e| e.is_pinned.unwrap_or(false)) {
        crate::append_error_log(format!("env: selecting pinned environment: {}", e.id));
        return Some(e.clone());
    }
    // Highest task_count as heuristic
    if let Some(e) = envs
        .iter()
        .max_by_key(|e| e.task_count.unwrap_or(0))
        .or_else(|| envs.first())
    {
        crate::append_error_log(format!("env: selecting by task_count/first: {}", e.id));
        return Some(e.clone());
    }
    None
}

async fn get_json<T: serde::de::DeserializeOwned>(
    url: &str,
    headers: &HeaderMap,
) -> anyhow::Result<T> {
    let http = reqwest::Client::builder().build()?;
    let res = http.get(url).headers(headers.clone()).send().await?;
    let status = res.status();
    let ct = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = res.text().await.unwrap_or_default();
    crate::append_error_log(format!("env: status={status} content-type={ct}"));
    if !status.is_success() {
        anyhow::bail!("GET {url} failed: {status}; content-type={ct}; body={body}");
    }
    let parsed = serde_json::from_str::<T>(&body).map_err(|e| {
        anyhow::anyhow!("Decode error for {url}: {e}; content-type={ct}; body={body}")
    })?;
    Ok(parsed)
}

fn get_git_origins() -> Vec<String> {
    // Prefer: git config --get-regexp remote\..*\.url
    let out = std::process::Command::new("git")
        .args(["config", "--get-regexp", "remote\\..*\\.url"])
        .output();
    if let Ok(ok) = out
        && ok.status.success()
    {
        let s = String::from_utf8_lossy(&ok.stdout);
        let mut urls = Vec::new();
        for line in s.lines() {
            if let Some((_, url)) = line.split_once(' ') {
                urls.push(url.trim().to_string());
            }
        }
        if !urls.is_empty() {
            return uniq(urls);
        }
    }
    // Fallback: git remote -v
    let out = std::process::Command::new("git")
        .args(["remote", "-v"])
        .output();
    if let Ok(ok) = out
        && ok.status.success()
    {
        let s = String::from_utf8_lossy(&ok.stdout);
        let mut urls = Vec::new();
        for line in s.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                urls.push(parts[1].to_string());
            }
        }
        if !urls.is_empty() {
            return uniq(urls);
        }
    }
    Vec::new()
}

fn uniq(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v.dedup();
    v
}

fn parse_owner_repo(url: &str) -> Option<(String, String)> {
    // Normalize common prefixes and handle multiple SSH/HTTPS variants.
    let mut s = url.trim().to_string();
    // Drop protocol scheme for ssh URLs
    if let Some(rest) = s.strip_prefix("ssh://") {
        s = rest.to_string();
    }
    // Accept any user before @github.com (e.g., git@, org-123@)
    if let Some(idx) = s.find("@github.com:") {
        let rest = &s[idx + "@github.com:".len()..];
        let rest = rest.trim_start_matches('/').trim_end_matches(".git");
        let mut parts = rest.splitn(2, '/');
        let owner = parts.next()?.to_string();
        let repo = parts.next()?.to_string();
        crate::append_error_log(format!("env: parsed SSH GitHub origin => {owner}/{repo}"));
        return Some((owner, repo));
    }
    // HTTPS or git protocol
    for prefix in [
        "https://github.com/",
        "http://github.com/",
        "git://github.com/",
        "github.com/",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            let rest = rest.trim_start_matches('/').trim_end_matches(".git");
            let mut parts = rest.splitn(2, '/');
            let owner = parts.next()?.to_string();
            let repo = parts.next()?.to_string();
            crate::append_error_log(format!("env: parsed HTTP GitHub origin => {owner}/{repo}"));
            return Some((owner, repo));
        }
    }
    None
}

/// List environments for the current repo(s) with a fallback to the global list.
/// Returns a de-duplicated, sorted set suitable for the TUI modal.
pub async fn list_environments(
    base_url: &str,
    headers: &HeaderMap,
) -> anyhow::Result<Vec<crate::app::EnvironmentRow>> {
    let mut map: HashMap<String, crate::app::EnvironmentRow> = HashMap::new();

    // 1) By-repo lookup for each parsed GitHub origin
    let origins = get_git_origins();
    for origin in &origins {
        if let Some((owner, repo)) = parse_owner_repo(origin) {
            let url = if base_url.contains("/backend-api") {
                format!(
                    "{}/wham/environments/by-repo/{}/{}/{}",
                    base_url, "github", owner, repo
                )
            } else {
                format!(
                    "{}/api/codex/environments/by-repo/{}/{}/{}",
                    base_url, "github", owner, repo
                )
            };
            match get_json::<Vec<CodeEnvironment>>(&url, headers).await {
                Ok(list) => {
                    info!("env_tui: by-repo {}:{} -> {} envs", owner, repo, list.len());
                    for e in list {
                        let entry =
                            map.entry(e.id.clone())
                                .or_insert_with(|| crate::app::EnvironmentRow {
                                    id: e.id.clone(),
                                    label: e.label.clone(),
                                    is_pinned: e.is_pinned.unwrap_or(false),
                                    repo_hints: Some(format!("{owner}/{repo}")),
                                });
                        // Merge: keep label if present, or use new; accumulate pinned flag
                        if entry.label.is_none() {
                            entry.label = e.label.clone();
                        }
                        entry.is_pinned = entry.is_pinned || e.is_pinned.unwrap_or(false);
                        if entry.repo_hints.is_none() {
                            entry.repo_hints = Some(format!("{owner}/{repo}"));
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "env_tui: by-repo fetch failed for {}/{}: {}",
                        owner, repo, e
                    );
                }
            }
        }
    }

    // 2) Fallback to the full list; on error return what we have if any.
    let list_url = if base_url.contains("/backend-api") {
        format!("{base_url}/wham/environments")
    } else {
        format!("{base_url}/api/codex/environments")
    };
    match get_json::<Vec<CodeEnvironment>>(&list_url, headers).await {
        Ok(list) => {
            info!("env_tui: global list -> {} envs", list.len());
            for e in list {
                let entry = map
                    .entry(e.id.clone())
                    .or_insert_with(|| crate::app::EnvironmentRow {
                        id: e.id.clone(),
                        label: e.label.clone(),
                        is_pinned: e.is_pinned.unwrap_or(false),
                        repo_hints: None,
                    });
                if entry.label.is_none() {
                    entry.label = e.label.clone();
                }
                entry.is_pinned = entry.is_pinned || e.is_pinned.unwrap_or(false);
            }
        }
        Err(e) => {
            if map.is_empty() {
                return Err(e);
            } else {
                warn!(
                    "env_tui: global list failed; using by-repo results only: {}",
                    e
                );
            }
        }
    }

    let mut rows: Vec<crate::app::EnvironmentRow> = map.into_values().collect();
    rows.sort_by(|a, b| {
        // pinned first
        let p = b.is_pinned.cmp(&a.is_pinned);
        if p != std::cmp::Ordering::Equal {
            return p;
        }
        // then label (ci), then id
        let al = a.label.as_deref().unwrap_or("").to_lowercase();
        let bl = b.label.as_deref().unwrap_or("").to_lowercase();
        let l = al.cmp(&bl);
        if l != std::cmp::Ordering::Equal {
            return l;
        }
        a.id.cmp(&b.id)
    });
    Ok(rows)
}
