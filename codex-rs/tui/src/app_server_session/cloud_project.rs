//! Resolves a host project into credential-free GitHub checkout metadata.
//! Local paths never become cloud executor cwd values or public wire fields.

use codex_app_server_client::ProjectEnvironment;
use std::path::Path;

pub(super) async fn from_local_cwd(cwd: &Path) -> Option<ProjectEnvironment> {
    let origin = codex_git_utils::get_git_origin_url(cwd).await?;
    github_project(origin.as_str())
}

fn github_project(origin: &str) -> Option<ProjectEnvironment> {
    // A relative local path such as github.com/org/repo is not a network origin.
    if !origin.contains("://")
        && !origin.split_once(':').is_some_and(|(host, _)| {
            host.rsplit('@')
                .next()
                .is_some_and(|host| host.eq_ignore_ascii_case("github.com"))
        })
    {
        return None;
    }
    let mut canonical = codex_git_utils::canonicalize_git_remote_url(origin)?;
    canonical.make_ascii_lowercase();
    let (host, path) = canonical.split_once('/')?;
    if !(host == "github.com" || (origin.starts_with("ssh://") && host == "ssh.github.com:443")) {
        return None;
    }
    let (owner, repository) = path.split_once('/')?;
    if ![owner, repository].iter().all(|part| {
        !part.is_empty()
            && !matches!(*part, "." | "..")
            && part.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            })
    }) {
        return None;
    }
    Some(ProjectEnvironment {
        name: repository.to_owned(),
        cwd: Some(format!("/workspace/{owner}/{repository}")),
        origin_urls: vec![format!("https://github.com/{owner}/{repository}")],
    })
}

#[cfg(test)]
#[path = "cloud_project_tests.rs"]
mod tests;
