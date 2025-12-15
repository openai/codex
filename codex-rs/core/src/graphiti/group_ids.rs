use sha2::Digest;
use sha2::Sha256;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use url::Url;

pub const CANONICAL_GROUP_ID_HASH_CHARS: usize = 32;

pub fn make_canonical_group_id(scope: &str, key: &str) -> String {
    let key = key.trim();
    let fallback_key;
    let key = if key.is_empty() {
        fallback_key = format!("no-{scope}-key");
        fallback_key.as_str()
    } else {
        key
    };

    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    let hex = hex_encode(digest.as_slice());
    format!("graphiti_{scope}_{}", &hex[..CANONICAL_GROUP_ID_HASH_CHARS])
}

pub async fn git_origin_url(repo_root: &Path, timeout: Duration) -> Option<String> {
    let out =
        run_git_output_with_timeout(repo_root, &["remote", "get-url", "origin"], timeout).await?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8(out.stdout).ok()?;
    let url = url.trim();
    (!url.is_empty()).then(|| url.to_string())
}

pub fn github_repo_key_from_remote_url(remote_url: &str) -> Option<String> {
    let remote_url = remote_url.trim();
    if remote_url.is_empty() {
        return None;
    }

    let normalized = if remote_url.contains("://") {
        remote_url.to_string()
    } else {
        // Normalize git shorthand syntax (git@github.com:org/repo.git) into an explicit ssh:// url.
        let (user_host, path) = remote_url.split_once(':')?;
        if !user_host.contains('@') || user_host.contains('/') {
            return None;
        }
        format!("ssh://{user_host}/{path}")
    };

    let url = Url::parse(&normalized).ok()?;
    let host = url.host_str()?.to_lowercase();

    let path = url.path().trim_matches('/');
    let mut segments = path.split('/');
    let org = segments.next()?.trim();
    let repo = segments.next()?.trim();
    if org.is_empty() || repo.is_empty() {
        return None;
    }
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    if repo.is_empty() {
        return None;
    }

    let nwo = format!("{}/{}", org.to_lowercase(), repo.to_lowercase());
    Some(format!("github_repo:{host}/{nwo}"))
}

pub async fn derive_user_scope_key(timeout: Duration) -> Option<String> {
    let child = Command::new("gh")
        .args(["auth", "status"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let out = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .ok()?
        .ok()?;
    if !out.status.success() {
        return None;
    }

    let stdout = String::from_utf8(out.stdout).ok()?;
    for line in stdout.lines() {
        let line = line.trim();
        if !line.contains("Logged in to") {
            continue;
        }
        let Some((_before, after)) = line.split_once(" as ") else {
            continue;
        };
        let login = after
            .trim()
            .split(|ch: char| ch.is_whitespace() || ch == '(')
            .next()
            .unwrap_or("")
            .trim();
        if login.is_empty() {
            continue;
        }
        return Some(format!("github_login:{}", login.to_lowercase()));
    }

    None
}

async fn run_git_output_with_timeout(
    repo_root: &Path,
    args: &[&str],
    timeout: Duration,
) -> Option<std::process::Output> {
    let child = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .ok()?
        .ok()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn canonical_group_id_is_stable_and_prefixed() {
        let id1 = make_canonical_group_id("workspace", "github_repo:github.com/org/repo");
        let id2 = make_canonical_group_id("workspace", "github_repo:github.com/org/repo");
        assert_eq!(id1, id2);
        assert!(id1.starts_with("graphiti_workspace_"));
        assert_eq!(id1.len(), "graphiti_workspace_".len() + 32);
    }

    #[test]
    fn github_repo_key_from_remote_url_parses_common_git_forms() {
        assert_eq!(
            github_repo_key_from_remote_url("git@github.com:Example/Repo.git"),
            Some("github_repo:github.com/example/repo".to_string())
        );
        assert_eq!(
            github_repo_key_from_remote_url("https://github.com/Example/Repo.git"),
            Some("github_repo:github.com/example/repo".to_string())
        );
    }
}
