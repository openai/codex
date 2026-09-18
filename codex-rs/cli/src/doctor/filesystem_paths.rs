//! Checks literal sandbox grants without traversing directory trees or probing deny rules.
//!
//! Filesystem calls run in disposable copies of this executable: a blocked mount
//! must not occupy a Tokio blocking worker and delay doctor's runtime shutdown.
//! Budgets bound helper waits, not synchronous process launch; policy is unchanged.
//! Windows only lists paths: even local-looking paths can redirect to network shares.
//! Restricted-read policies also only list paths: lexical checks cannot constrain symlinks.

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use codex_config::ConfigPathContext;
use codex_config::format_config_layer_source;
use codex_config::permissions_toml::FilesystemPermissionToml;
use codex_core::config::Config;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSandboxPolicyContext;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use tokio::process::Command;

use super::CheckStatus;
use super::DoctorCheck;
use super::DoctorIssue;

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(8);
const SLOW_PROBE: Duration = Duration::from_secs(1);
const MAX_PATHS: usize = 32;

pub(super) async fn check(config: &Config) -> DoctorCheck {
    let mut check = DoctorCheck::new(
        "sandbox.filesystem_paths",
        "sandbox",
        CheckStatus::Ok,
        "configured filesystem paths respond promptly",
    );
    // Resolve policy semantics lexically, without querying the configured paths.
    let policy = config
        .permissions
        .permission_profile()
        .file_system_sandbox_policy();
    let cwd = PathUri::from_abs_path(&config.cwd);
    let roots = config.effective_workspace_roots();
    let home = AbsolutePathBufGuard::home_directory()
        .and_then(|home| PathUri::from_host_native_path(home).ok());
    let temporary_directories = std::env::var_os("TMPDIR")
        .filter(|path| !path.is_empty())
        .and_then(|path| AbsolutePathBuf::from_absolute_path(path).ok())
        .map(PathUri::from)
        .into_iter()
        .collect::<Vec<_>>();
    let context = FileSystemSandboxPolicyContext {
        cwd: &cwd,
        workspace_roots: &roots,
        user_home_dir: home.as_ref(),
        temporary_directories: Some(&temporary_directories),
    };
    let paths = literal_paths(&policy, &context);
    if paths.is_empty() {
        check.summary = "no explicit filesystem paths to probe".to_string();
        return check;
    }
    let read_restricted = !policy.has_full_disk_read_access();
    if read_restricted {
        check.summary =
            "configured filesystem paths listed; read restrictions prevent probes".to_string();
    } else if cfg!(windows) {
        check.summary =
            "configured filesystem paths listed; probes disabled on Windows".to_string();
    }
    if let Some(profile) = config.permissions.active_permission_profile() {
        check
            .details
            .push(format!("permission profile: {}", profile.id));
    }
    check
        .details
        .push("operation: resolve paths only; read/write access is not tested".to_string());
    check
        .details
        .push("probe budgets: 2 seconds per path, 8 seconds total, at most 32 paths".to_string());
    let started = Instant::now();
    let mut checked = 0;
    for (path, access) in paths.iter().take(MAX_PATHS) {
        let remaining = TOTAL_TIMEOUT.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            break;
        }
        let label = format!("path {}", checked + 1);
        let probe_started = Instant::now();
        let outcome = match path.to_abs_path() {
            Ok(_) if read_restricted => ProbeResult::SkippedReadRestrictions,
            Ok(_) if cfg!(windows) => ProbeResult::SkippedWindows,
            Ok(native) => {
                let executable = match std::env::current_exe() {
                    Ok(executable) => executable,
                    Err(_) => {
                        check.status = CheckStatus::Warning;
                        check.summary =
                            "filesystem path probes unavailable: cannot locate Codex executable"
                                .to_string();
                        return check;
                    }
                };
                let mut command = Command::new(&executable);
                command
                    .arg("doctor")
                    .arg("--probe-filesystem-path")
                    .arg(native.as_path());
                probe(&mut command, remaining.min(PROBE_TIMEOUT)).await
            }
            Err(_) => ProbeResult::Unsupported,
        };
        let slow = probe_started.elapsed() >= SLOW_PROBE;
        checked += 1;
        let access = access
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        check.details.push(format!(
            "{label}: {} ({access}); {}",
            path.inferred_native_path_string(),
            outcome.description()
        ));
        if slow {
            check.details.push(format!(
                "{label} latency: over 1 second (includes helper startup)"
            ));
        }
        check
            .details
            .push(format!("{label} source: {}", source(config, path)));
        if !matches!(
            outcome,
            ProbeResult::Resolved
                | ProbeResult::Missing
                | ProbeResult::SkippedWindows
                | ProbeResult::SkippedReadRestrictions
        ) || slow
        {
            check.status = CheckStatus::Warning;
            check.summary =
                "some configured filesystem paths are slow or could not be checked".to_string();
            check.issues.push(
                DoctorIssue::new(CheckStatus::Warning, format!("{label}: {}{}", outcome.description(), if slow { "; slow path probe" } else { "" }))
                    .field(label)
                    .remedy("Check this path's mount and host permissions. Ask your administrator whether the entry is needed on this machine; do not remove required restrictions."),
            );
        }
    }
    if checked < paths.len() {
        check.status = CheckStatus::Warning;
        check.summary = "filesystem path check is incomplete".to_string();
        check.details.push(format!(
            "unchecked paths: {} (probe budget exhausted)",
            paths.len() - checked
        ));
    }
    check
        .details
        .push(format!("paths checked: {checked} of {}", paths.len()));
    check
}

fn literal_paths(
    policy: &FileSystemSandboxPolicy,
    context: &FileSystemSandboxPolicyContext<'_>,
) -> Vec<(PathUri, BTreeSet<FileSystemAccessMode>)> {
    let mut paths = HashMap::<_, BTreeSet<_>>::new();
    let denials = ReadDenyMatcher::from_context(policy, context);
    for entry in &policy.entries {
        // Deny rules can contain private managed-policy paths. Doctor only
        // reports their counts in the existing sandbox check.
        if entry.access == FileSystemAccessMode::Deny {
            continue;
        }
        match &entry.path {
            FileSystemPath::Path { path } => {
                if policy.can_read_path(path, context)
                    && !denials
                        .as_ref()
                        .is_some_and(|matcher| matcher.is_read_denied_uri(path, context))
                {
                    paths.entry(path.clone()).or_default().insert(entry.access);
                }
            }
            FileSystemPath::GlobPattern { .. } | FileSystemPath::Special { .. } => {}
        }
    }
    let mut paths = paths.into_iter().collect::<Vec<_>>();
    paths.sort_by_cached_key(|(path, _)| path.to_string());
    paths
}

fn source(config: &Config, path: &PathUri) -> String {
    let active = config.permissions.active_permission_profile();
    let context = ConfigPathContext::new(
        PathConvention::native(),
        Some(PathUri::from_abs_path(&config.cwd)),
        AbsolutePathBufGuard::home_directory()
            .and_then(|home| PathUri::from_host_native_path(home).ok()),
    );
    let effective = config.config_layer_stack.effective_config();
    let managed = config
        .config_layer_stack
        .requirements_toml()
        .permissions
        .as_ref();
    let matches_path = |base: &str, subpath: Option<&str>| {
        let base = if subpath.is_none() {
            base.strip_suffix("/**").unwrap_or(base)
        } else {
            base
        };
        let Ok(base) = context.resolve_path(base) else {
            return false;
        };
        match subpath.map(|subpath| subpath.strip_suffix("/**").unwrap_or(subpath)) {
            None | Some(".") => base == *path,
            Some(subpath) => context
                .resolve_against(subpath, &base)
                .is_ok_and(|resolved| resolved == *path),
        }
    };
    let mut profiles = Vec::new();
    let mut next = active.as_ref().map(|profile| profile.id.as_str());
    while let Some(profile) = next {
        if profiles.contains(&profile) {
            break;
        }
        profiles.push(profile);
        next = effective
            .get("permissions")
            .and_then(|permissions| permissions.get(profile))
            .and_then(|profile| profile.get("extends"))
            .and_then(toml::Value::as_str)
            .or_else(|| managed?.profiles.get(profile)?.extends.as_deref());
    }
    for profile in profiles {
        // Managed grants have no config-layer origin and can shadow a parent's grant.
        if managed
            .and_then(|managed| managed.profiles.get(profile))
            .and_then(|profile| profile.filesystem.as_ref())
            .is_some_and(|filesystem| {
                filesystem.entries.iter().any(|(base, entry)| match entry {
                    FilesystemPermissionToml::Access(access) => {
                        *access != FileSystemAccessMode::Deny
                            && matches_path(base, /*subpath*/ None)
                    }
                    FilesystemPermissionToml::Scoped(entries) => {
                        entries.iter().any(|(subpath, access)| {
                            *access != FileSystemAccessMode::Deny
                                && matches_path(base, Some(subpath))
                        })
                    }
                })
            })
        {
            break;
        }
        let origins = config
            .config_layer_stack
            .origins_with_path_filter(|segments| {
                if segments.len() < 4 || segments[0] != "permissions" || segments[2] != "filesystem"
                {
                    return false;
                }
                if profile != segments[1] {
                    return false;
                }
                // Origins can retain leaves removed by a table/scalar replacement.
                if !matches!(
                    segments
                        .iter()
                        .try_fold(&effective, |value, segment| value.get(segment))
                        .and_then(toml::Value::as_str),
                    Some("read" | "write")
                ) {
                    return false;
                }
                matches_path(&segments[3], segments.get(4).map(String::as_str))
            });
        let sources = origins
            .into_iter()
            .map(|(key, origin)| {
                format!(
                    "{key} in {}",
                    format_config_layer_source(&origin.name, codex_config::CONFIG_TOML_FILE)
                )
            })
            .collect::<BTreeSet<_>>();
        if !sources.is_empty() {
            return sources.into_iter().collect::<Vec<_>>().join("; ");
        }
    }
    // Compiled requirements, legacy config, and runtime-added entries do not
    // retain per-entry provenance. Do not misattribute them to a config file.
    "effective filesystem policy (entry provenance unavailable)".to_string()
}

#[derive(Debug, PartialEq, Eq)]
enum ProbeResult {
    Resolved,
    Missing,
    Denied,
    TimedOut,
    Failed,
    Unsupported,
    SkippedWindows,
    SkippedReadRestrictions,
}

impl ProbeResult {
    fn description(&self) -> &'static str {
        match self {
            Self::Resolved => "path resolved (read/write access not tested)",
            Self::Missing => "missing (may be intentional)",
            Self::Denied => "access denied to doctor",
            Self::TimedOut => "path probe timed out",
            Self::Failed => "path probe failed",
            Self::Unsupported => "path is not native to this host; not probed",
            Self::SkippedWindows => "not probed on Windows (network authentication risk)",
            Self::SkippedReadRestrictions => "not probed (filesystem read restrictions apply)",
        }
    }
}

async fn probe(command: &mut Command, budget: Duration) -> ProbeResult {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(/*kill_on_drop*/ true);
    let Ok(mut child) = command.spawn() else {
        return ProbeResult::Failed;
    };
    match tokio::time::timeout(budget, child.wait()).await {
        Ok(Ok(status)) => match status.code() {
            Some(0) => ProbeResult::Resolved,
            Some(2) => ProbeResult::Missing,
            Some(3) => ProbeResult::Denied,
            Some(5) => ProbeResult::SkippedWindows,
            _ => ProbeResult::Failed,
        },
        Ok(Err(_)) => ProbeResult::Failed,
        Err(_) => {
            // Do not await termination: an uninterruptible filesystem syscall
            // can delay even SIGKILL. Tokio reaps the child when it can exit.
            let _ = child.start_kill();
            ProbeResult::TimedOut
        }
    }
}

pub(super) fn probe_exit_code(path: &Path) -> i32 {
    // Even drive-qualified paths can traverse reparse points to network shares.
    // Do not initiate filesystem access on Windows from this unsandboxed helper.
    if cfg!(windows) {
        return 5;
    }
    match std::fs::canonicalize(path) {
        Ok(_) => 0,
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => 2,
            io::ErrorKind::PermissionDenied => 3,
            _ => 4,
        },
    }
}

#[cfg(test)]
#[path = "filesystem_paths_tests.rs"]
mod tests;
