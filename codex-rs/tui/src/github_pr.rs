use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;

use serde::Deserialize;
use tokio::process::Command;
use tokio::time::Duration;
use tokio::time::timeout;

const GH_LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GithubPullRequest {
    pub(crate) number: u64,
    pub(crate) url: String,
}

impl GithubPullRequest {
    pub(crate) fn label(&self) -> String {
        format!("PR #{}", self.number)
    }
}

pub(crate) fn gh_available() -> bool {
    resolve_gh_path().is_some()
}

pub(crate) async fn lookup_current_branch_pull_request(cwd: &Path) -> Option<GithubPullRequest> {
    let gh_path = resolve_gh_path()?;
    let mut command = Command::new(&gh_path);
    command
        .args(["pr", "view", "--json", "number,url"])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let output = match timeout(GH_LOOKUP_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(_)) | Err(_) => return None,
    };

    if !output.status.success() {
        return None;
    }

    parse_gh_pr_view_json(&output.stdout)
}

fn resolve_gh_path() -> Option<PathBuf> {
    resolve_gh_path_from_path(std::env::var_os("PATH").as_deref())
}

fn resolve_gh_path_from_path(path_env: Option<&OsStr>) -> Option<PathBuf> {
    path_env
        .into_iter()
        .flat_map(std::env::split_paths)
        .flat_map(|dir| gh_executable_names().map(move |name| dir.join(name)))
        .find(|path| is_executable_file(path))
}

#[cfg(windows)]
fn gh_executable_names() -> impl Iterator<Item = &'static str> {
    ["gh.exe", "gh.cmd", "gh.bat", "gh"].into_iter()
}

#[cfg(not(windows))]
fn gh_executable_names() -> impl Iterator<Item = &'static str> {
    ["gh"].into_iter()
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn parse_gh_pr_view_json(bytes: &[u8]) -> Option<GithubPullRequest> {
    #[derive(Deserialize)]
    struct GhPrView {
        number: u64,
        url: String,
    }

    let view = serde_json::from_slice::<GhPrView>(bytes).ok()?;
    (!view.url.trim().is_empty()).then_some(GithubPullRequest {
        number: view.number,
        url: view.url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_valid_pr_view_json() {
        assert_eq!(
            parse_gh_pr_view_json(br#"{"number":123,"url":"https://github.com/o/r/pull/123"}"#),
            Some(GithubPullRequest {
                number: 123,
                url: "https://github.com/o/r/pull/123".to_string(),
            })
        );
    }

    #[test]
    fn rejects_incomplete_pr_view_json() {
        assert_eq!(
            parse_gh_pr_view_json(br#"{"url":"https://example.com"}"#),
            None
        );
        assert_eq!(parse_gh_pr_view_json(br#"{"number":123}"#), None);
        assert_eq!(parse_gh_pr_view_json(br#"{"number":123,"url":""}"#), None);
        assert_eq!(parse_gh_pr_view_json(b"not json"), None);
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = path.metadata().expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("set permissions");
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Path) {}

    #[cfg(windows)]
    fn gh_test_file_name() -> &'static str {
        "gh.exe"
    }

    #[cfg(not(windows))]
    fn gh_test_file_name() -> &'static str {
        "gh"
    }

    #[test]
    fn resolves_gh_from_path_env() {
        let temp = tempfile::tempdir().expect("tempdir");
        let gh = temp.path().join(gh_test_file_name());
        std::fs::write(&gh, "").expect("write gh");
        make_executable(&gh);

        let resolved = resolve_gh_path_from_path(Some(temp.path().as_os_str()));

        assert_eq!(resolved, Some(gh));
    }
}
