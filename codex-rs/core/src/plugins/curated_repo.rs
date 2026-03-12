use crate::default_client::build_reqwest_client;
use reqwest::Client;
use serde::Deserialize;
use std::fs;
use std::io::Cursor;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use zip::ZipArchive;

const GITHUB_API_BASE_URL: &str = "https://api.github.com";
const GITHUB_API_ACCEPT_HEADER: &str = "application/vnd.github+json";
const GITHUB_API_VERSION_HEADER: &str = "2022-11-28";
const OPENAI_PLUGINS_OWNER: &str = "openai";
const OPENAI_PLUGINS_REPO: &str = "plugins";
const CURATED_PLUGINS_RELATIVE_DIR: &str = ".tmp/plugins";
const CURATED_PLUGINS_SHA_FILE: &str = ".tmp/plugins.sha";
const CURATED_PLUGINS_HTTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct GitHubRepositorySummary {
    default_branch: String,
}

#[derive(Debug, Deserialize)]
struct GitHubGitRefSummary {
    object: GitHubGitRefObject,
}

#[derive(Debug, Deserialize)]
struct GitHubGitRefObject {
    sha: String,
}

pub(crate) fn curated_plugins_repo_path(codex_home: &Path) -> PathBuf {
    codex_home.join(CURATED_PLUGINS_RELATIVE_DIR)
}

pub(crate) fn read_curated_plugins_sha(codex_home: &Path) -> Option<String> {
    read_sha_file(codex_home.join(CURATED_PLUGINS_SHA_FILE).as_path())
}

pub(crate) fn sync_openai_plugins_repo(codex_home: &Path) -> Result<String, String> {
    sync_openai_plugins_repo_with_api_base_url(codex_home, GITHUB_API_BASE_URL)
}

fn sync_openai_plugins_repo_with_api_base_url(
    codex_home: &Path,
    api_base_url: &str,
) -> Result<String, String> {
    let repo_path = curated_plugins_repo_path(codex_home);
    let sha_path = codex_home.join(CURATED_PLUGINS_SHA_FILE);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to create curated plugins sync runtime: {err}"))?;
    let remote_sha = runtime.block_on(fetch_curated_repo_remote_sha(api_base_url))?;
    let local_sha = read_sha_file(&sha_path);

    if local_sha.as_deref() == Some(remote_sha.as_str()) && repo_path.is_dir() {
        return Ok(remote_sha);
    }

    let Some(parent) = repo_path.parent() else {
        return Err(format!(
            "failed to determine curated plugins parent directory for {}",
            repo_path.display()
        ));
    };
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "failed to create curated plugins parent directory {}: {err}",
            parent.display()
        )
    })?;

    let clone_dir = tempfile::Builder::new()
        .prefix("plugins-clone-")
        .tempdir_in(parent)
        .map_err(|err| {
            format!(
                "failed to create temporary curated plugins directory in {}: {err}",
                parent.display()
            )
        })?;
    let cloned_repo_path = clone_dir.path().join("repo");
    let zipball_bytes = runtime.block_on(fetch_curated_repo_zipball(api_base_url, &remote_sha))?;
    extract_zipball_to_dir(&zipball_bytes, &cloned_repo_path)?;

    if !cloned_repo_path
        .join(".agents/plugins/marketplace.json")
        .is_file()
    {
        return Err(format!(
            "curated plugins archive missing marketplace manifest at {}",
            cloned_repo_path
                .join(".agents/plugins/marketplace.json")
                .display()
        ));
    }

    if repo_path.exists() {
        let backup_dir = tempfile::Builder::new()
            .prefix("plugins-backup-")
            .tempdir_in(parent)
            .map_err(|err| {
                format!(
                    "failed to create curated plugins backup directory in {}: {err}",
                    parent.display()
                )
            })?;
        let backup_repo_path = backup_dir.path().join("repo");

        fs::rename(&repo_path, &backup_repo_path).map_err(|err| {
            format!(
                "failed to move previous curated plugins repo out of the way at {}: {err}",
                repo_path.display()
            )
        })?;

        if let Err(err) = fs::rename(&cloned_repo_path, &repo_path) {
            let rollback_result = fs::rename(&backup_repo_path, &repo_path);
            return match rollback_result {
                Ok(()) => Err(format!(
                    "failed to activate new curated plugins repo at {}: {err}",
                    repo_path.display()
                )),
                Err(rollback_err) => {
                    let backup_path = backup_dir.keep().join("repo");
                    Err(format!(
                        "failed to activate new curated plugins repo at {}: {err}; failed to restore previous repo (left at {}): {rollback_err}",
                        repo_path.display(),
                        backup_path.display()
                    ))
                }
            };
        }
    } else {
        fs::rename(&cloned_repo_path, &repo_path).map_err(|err| {
            format!(
                "failed to activate curated plugins repo at {}: {err}",
                repo_path.display()
            )
        })?;
    }

    if let Some(parent) = sha_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create curated plugins sha directory {}: {err}",
                parent.display()
            )
        })?;
    }
    fs::write(&sha_path, format!("{remote_sha}\n")).map_err(|err| {
        format!(
            "failed to write curated plugins sha file {}: {err}",
            sha_path.display()
        )
    })?;

    Ok(remote_sha)
}

async fn fetch_curated_repo_remote_sha(api_base_url: &str) -> Result<String, String> {
    let api_base_url = api_base_url.trim_end_matches('/');
    let repo_url = format!("{api_base_url}/repos/{OPENAI_PLUGINS_OWNER}/{OPENAI_PLUGINS_REPO}");
    let client = build_reqwest_client();
    let repo_body = fetch_github_text(&client, &repo_url, "get curated plugins repository").await?;
    let repo_summary: GitHubRepositorySummary =
        serde_json::from_str(&repo_body).map_err(|err| {
            format!("failed to parse curated plugins repository response from {repo_url}: {err}")
        })?;
    if repo_summary.default_branch.is_empty() {
        return Err(format!(
            "curated plugins repository response from {repo_url} did not include a default branch"
        ));
    }

    let git_ref_url = format!("{repo_url}/git/ref/heads/{}", repo_summary.default_branch);
    let git_ref_body =
        fetch_github_text(&client, &git_ref_url, "get curated plugins HEAD ref").await?;
    let git_ref: GitHubGitRefSummary = serde_json::from_str(&git_ref_body).map_err(|err| {
        format!("failed to parse curated plugins ref response from {git_ref_url}: {err}")
    })?;
    if git_ref.object.sha.is_empty() {
        return Err(format!(
            "curated plugins ref response from {git_ref_url} did not include a HEAD sha"
        ));
    }

    Ok(git_ref.object.sha)
}

async fn fetch_curated_repo_zipball(
    api_base_url: &str,
    remote_sha: &str,
) -> Result<Vec<u8>, String> {
    let api_base_url = api_base_url.trim_end_matches('/');
    let repo_url = format!("{api_base_url}/repos/{OPENAI_PLUGINS_OWNER}/{OPENAI_PLUGINS_REPO}");
    let zipball_url = format!("{repo_url}/zipball/{remote_sha}");
    let client = build_reqwest_client();
    fetch_github_bytes(&client, &zipball_url, "download curated plugins archive").await
}

async fn fetch_github_text(client: &Client, url: &str, context: &str) -> Result<String, String> {
    let response = github_request(client, url)
        .send()
        .await
        .map_err(|err| format!("failed to {context} from {url}: {err}"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "{context} from {url} failed with status {status}: {body}"
        ));
    }
    Ok(body)
}

async fn fetch_github_bytes(client: &Client, url: &str, context: &str) -> Result<Vec<u8>, String> {
    let response = github_request(client, url)
        .send()
        .await
        .map_err(|err| format!("failed to {context} from {url}: {err}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|err| format!("failed to read {context} response from {url}: {err}"))?;
    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body);
        return Err(format!(
            "{context} from {url} failed with status {status}: {body_text}"
        ));
    }
    Ok(body.to_vec())
}

fn github_request(client: &Client, url: &str) -> reqwest::RequestBuilder {
    client
        .get(url)
        .timeout(CURATED_PLUGINS_HTTP_TIMEOUT)
        .header("accept", GITHUB_API_ACCEPT_HEADER)
        .header("x-github-api-version", GITHUB_API_VERSION_HEADER)
}

fn read_sha_file(sha_path: &Path) -> Option<String> {
    fs::read_to_string(sha_path)
        .ok()
        .map(|sha| sha.trim().to_string())
        .filter(|sha| !sha.is_empty())
}

fn extract_zipball_to_dir(bytes: &[u8], destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|err| {
        format!(
            "failed to create curated plugins extraction directory {}: {err}",
            destination.display()
        )
    })?;

    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|err| format!("failed to open curated plugins zip archive: {err}"))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| format!("failed to read curated plugins zip entry: {err}"))?;
        let Some(relative_path) = entry.enclosed_name() else {
            return Err(format!(
                "curated plugins zip entry `{}` escapes extraction root",
                entry.name()
            ));
        };

        let mut components = relative_path.components();
        let Some(Component::Normal(_)) = components.next() else {
            continue;
        };

        let output_relative = components.fold(PathBuf::new(), |mut path, component| {
            if let Component::Normal(segment) = component {
                path.push(segment);
            }
            path
        });
        if output_relative.as_os_str().is_empty() {
            continue;
        }

        let output_path = destination.join(&output_relative);
        if entry.is_dir() {
            fs::create_dir_all(&output_path).map_err(|err| {
                format!(
                    "failed to create curated plugins directory {}: {err}",
                    output_path.display()
                )
            })?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create curated plugins directory {}: {err}",
                    parent.display()
                )
            })?;
        }
        let mut output = fs::File::create(&output_path).map_err(|err| {
            format!(
                "failed to create curated plugins file {}: {err}",
                output_path.display()
            )
        })?;
        std::io::copy(&mut entry, &mut output).map_err(|err| {
            format!(
                "failed to write curated plugins file {}: {err}",
                output_path.display()
            )
        })?;
        apply_zip_permissions(&entry, &output_path)?;
    }

    Ok(())
}

#[cfg(unix)]
fn apply_zip_permissions(entry: &zip::read::ZipFile<'_>, output_path: &Path) -> Result<(), String> {
    let Some(mode) = entry.unix_mode() else {
        return Ok(());
    };
    fs::set_permissions(output_path, fs::Permissions::from_mode(mode)).map_err(|err| {
        format!(
            "failed to set permissions on curated plugins file {}: {err}",
            output_path.display()
        )
    })
}

#[cfg(not(unix))]
fn apply_zip_permissions(
    _entry: &zip::read::ZipFile<'_>,
    _output_path: &Path,
) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
#[path = "curated_repo_tests.rs"]
mod tests;
