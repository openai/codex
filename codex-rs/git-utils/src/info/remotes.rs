//! Collects sanitized fetch URLs from NUL-delimited Git configuration without contacting remotes.
//! Git resolves rewritten or reset URLs within the same collection deadline.

use std::collections::BTreeMap;
use std::path::Path;

use futures::FutureExt;
use futures::StreamExt;
use futures::stream;

use super::DISABLED_HOOKS_PATH;
use super::GIT_COMMAND_TIMEOUT;
use super::local_git_command;
use super::run_git_command_with_timeout;
use crate::SanitizedGitUrl;
use crate::git_process::run_git_command_with_timeout_output;

const MAX_CONCURRENT_REMOTE_LOOKUPS: usize = 8;

/// Collect fetch remotes in a multi-root-friendly format: {"origin": "https://..."}.
pub async fn get_git_remote_urls(cwd: &Path) -> Option<BTreeMap<String, SanitizedGitUrl>> {
    let is_git_repo = run_git_command_with_timeout(&["rev-parse", "--git-dir"], cwd)
        .await?
        .status
        .success();
    if !is_git_repo {
        return None;
    }

    get_git_remote_urls_assume_git_repo(cwd).await
}

/// Collect fetch remotes without checking whether `cwd` is in a git repo.
pub async fn get_git_remote_urls_assume_git_repo(
    cwd: &Path,
) -> Option<BTreeMap<String, SanitizedGitUrl>> {
    get_git_remote_urls_with_progress(cwd, |_| {}).await
}

/// Publishes verified fetch URLs as they become available, keeping previously resolved names.
pub async fn observe_git_remote_urls(
    cwd: &Path,
    updates: &tokio::sync::watch::Sender<BTreeMap<String, SanitizedGitUrl>>,
) {
    get_git_remote_urls_with_progress(cwd, |urls| {
        updates.send_modify(|known| known.extend(urls.clone()));
    })
    .await;
}

/// Publishes unambiguous configured URLs, then each successful native resolution.
async fn get_git_remote_urls_with_progress(
    cwd: &Path,
    mut publish_verified: impl FnMut(&BTreeMap<String, SanitizedGitUrl>),
) -> Option<BTreeMap<String, SanitizedGitUrl>> {
    let deadline = tokio::time::Instant::now() + GIT_COMMAND_TIMEOUT;
    // `git remote` ignores GIT_CONFIG, but `git config` treats it as an explicit
    // file. Clear it so the machine-readable query observes the same configuration.
    let mut command = local_git_command(
        Path::new("git"),
        &["config", "--null", "--get-regexp", r"^(remote\.|url\.)"],
        cwd,
        crate::FsmonitorOverride::Disabled,
    );
    command.env_remove("GIT_CONFIG");
    let config = run_git_command_with_timeout_output(&mut command, GIT_COMMAND_TIMEOUT).await?;
    if !config.status.success() {
        return None;
    }
    let mut configured = BTreeMap::<&str, Vec<&[u8]>>::new();
    let mut rewrite_prefixes = Vec::new();
    for entry in config.stdout.split(|byte| *byte == b'\0') {
        if entry.is_empty() {
            continue;
        }
        let separator = entry
            .iter()
            .position(|byte| *byte == b'\n')
            .unwrap_or(entry.len());
        let (key, value) = entry.split_at(separator);
        if let Some(name) = key
            .strip_prefix(b"remote.")
            .and_then(|key| key.strip_suffix(b".url"))
            .and_then(|name| std::str::from_utf8(name).ok())
        {
            // Git interprets an empty remote subsection as the name `.url`.
            let name = if name.is_empty() { ".url" } else { name };
            configured
                .entry(name)
                .or_default()
                .push(value.strip_prefix(b"\n")?);
        } else if key
            .strip_prefix(b"url.")
            .and_then(|key| key.strip_suffix(b".insteadof"))
            .is_some()
        {
            rewrite_prefixes.push(value.strip_prefix(b"\n")?);
        }
    }
    let mut remotes = BTreeMap::new();
    let mut pending = BTreeMap::new();
    for (name, values) in configured {
        let first = *values.first()?;
        let has_empty = values.iter().any(|value| value.is_empty());
        let rewritten = rewrite_prefixes
            .iter()
            .any(|prefix| first.starts_with(prefix));
        if has_empty || rewritten {
            pending.insert(
                name.to_owned(),
                values.last().is_some_and(|value| value.is_empty()),
            );
            continue;
        }
        if let Ok(raw) = std::str::from_utf8(first)
            && !raw.contains(['\n', '\r'])
            && !raw.trim().is_empty()
            && let Ok(url) = SanitizedGitUrl::try_from(raw.trim_start())
        {
            remotes.insert(name.to_owned(), url);
        }
    }
    if !remotes.is_empty() {
        publish_verified(&remotes);
    }
    // Let Git implement insteadOf precedence and version-dependent URL resets.
    let origin = pending.remove_entry("origin");
    let (known, ambiguous): (Vec<_>, Vec<_>) = pending.into_iter().partition(|(_, empty)| !empty);
    let resets = git_resets_empty_fetch_url(cwd).shared();
    let resolved = stream::iter(origin.into_iter().chain(known).chain(ambiguous))
        .map(|(name, last_empty)| {
            let resets = resets.clone();
            async move {
                // A trailing empty URL clears the list on newer Git. Older Git
                // resolves its first configured URL, including insteadOf rewrites.
                if last_empty && resets.await? {
                    return None;
                }
                // --get-url expands the configured fetch URL locally.
                let output =
                    run_git_command_with_timeout(&["ls-remote", "--get-url", "--", &name], cwd)
                        .await?;
                if !output.status.success() {
                    return None;
                }
                let stdout = String::from_utf8(output.stdout).ok()?;
                let mut lines = stdout.lines();
                let url = lines.next()?;
                if lines.next().is_some() || url.contains('\r') || url.trim().is_empty() {
                    return None;
                }
                let url = SanitizedGitUrl::try_from(url.trim_start()).ok()?;
                Some((name, url))
            }
        })
        .buffer_unordered(MAX_CONCURRENT_REMOTE_LOOKUPS)
        .take_until(tokio::time::sleep_until(deadline))
        .filter_map(std::future::ready);
    let mut resolved = std::pin::pin!(resolved);
    while let Some(remote) = resolved.next().await {
        let update = BTreeMap::from([remote]);
        publish_verified(&update);
        remotes.extend(update);
    }
    (!remotes.is_empty()).then_some(remotes)
}

pub(crate) async fn git_resets_empty_fetch_url(cwd: &Path) -> Option<bool> {
    let null_device = DISABLED_HOOKS_PATH;
    let mut command = local_git_command(
        Path::new("git"),
        &[
            "--git-dir",
            null_device,
            "-c",
            "remote.codex-reset-probe.url=before-reset",
            "-c",
            "remote.codex-reset-probe.url=",
            "ls-remote",
            "--get-url",
            "--",
            "codex-reset-probe",
        ],
        cwd,
        crate::FsmonitorOverride::Disabled,
    );
    command
        .env("GIT_CONFIG_GLOBAL", null_device)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_COUNT", "0")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_WORK_TREE");
    let output = run_git_command_with_timeout_output(&mut command, GIT_COMMAND_TIMEOUT).await?;
    if !output.status.success() {
        return None;
    }
    match std::str::from_utf8(&output.stdout).ok()?.trim_end() {
        "before-reset" => Some(false),
        "codex-reset-probe" => Some(true),
        _ => None,
    }
}

#[cfg(test)]
#[path = "remotes_tests.rs"]
mod tests;
