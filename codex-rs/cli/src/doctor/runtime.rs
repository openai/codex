//! Captures how this Codex process was launched.
//!
//! Runtime diagnostics answer provenance questions that are hard to infer from
//! user reports: which binary is running, which install channel it resembles,
//! which platform it targets, and whether the search command comes from bundled
//! standalone resources or from PATH.

use std::env;
use std::process::Command;

use codex_install_context::InstallContext;

use super::CheckStatus;
use super::DoctorCheck;
use super::describe_install_context;
use super::doctor_install_context;
use super::push_path_detail;

/// Builds the process provenance row for the current Codex executable.
///
/// This check is informational and should not fail on its own; inconsistent
/// install state is reported by the installation and update checks instead.
pub(super) fn runtime_check() -> DoctorCheck {
    let current_exe = env::current_exe().ok();
    let install_context = doctor_install_context(current_exe.as_deref());
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    let platform = format!("{os}-{arch}");
    let install_method = install_method_name(&install_context);
    let mut details = vec![
        format!("version: {}", env!("CARGO_PKG_VERSION")),
        format!("platform: {platform}"),
        format!(
            "install method: {}",
            describe_install_context(&install_context)
        ),
        format!("commit: {}", build_commit()),
    ];
    push_path_detail(&mut details, "current executable", current_exe.as_deref());

    DoctorCheck::new(
        "runtime.provenance",
        "runtime",
        CheckStatus::Ok,
        format!("running {install_method} on {platform}"),
    )
    .details(details)
}

/// Verifies that the search command selected by the install context is usable.
///
/// Standalone installs should point at a bundled ripgrep binary, while local or
/// package-managed installs usually resolve rg from PATH. A warning here means
/// features that depend on file search may degrade even when the CLI launches.
pub(super) fn search_check() -> DoctorCheck {
    let current_exe = env::current_exe().ok();
    let install_context = doctor_install_context(current_exe.as_deref());
    let rg_command = install_context.rg_command();
    let provider = search_provider(&install_context);
    let mut details = vec![
        format!("search command: {}", rg_command.display()),
        format!("search provider: {provider}"),
    ];

    let status = if rg_command.components().count() > 1 {
        match std::fs::metadata(&rg_command) {
            Ok(metadata) if metadata.is_file() => {
                details.push("search command readiness: file exists".to_string());
                CheckStatus::Ok
            }
            Ok(_) => {
                details.push("search command readiness: path is not a file".to_string());
                CheckStatus::Warning
            }
            Err(err) => {
                details.push(format!("search command readiness: {err}"));
                CheckStatus::Warning
            }
        }
    } else {
        match Command::new(&rg_command).arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("rg version unknown")
                    .to_string();
                details.push(format!("search command readiness: {version}"));
                CheckStatus::Ok
            }
            Ok(output) => {
                details.push(format!(
                    "search command readiness: exited with status {}",
                    output.status
                ));
                CheckStatus::Warning
            }
            Err(err) => {
                details.push(format!("search command readiness: {err}"));
                CheckStatus::Warning
            }
        }
    };

    let summary = match status {
        CheckStatus::Ok => format!("search is OK ({provider})"),
        CheckStatus::Warning => "search command could not be verified".to_string(),
        CheckStatus::Fail => unreachable!(),
    };
    let mut check = DoctorCheck::new("runtime.search", "search", status, summary).details(details);
    if status != CheckStatus::Ok {
        check = check.remediation("Install ripgrep or repair the bundled standalone resources.");
    }
    check
}

fn install_method_name(context: &InstallContext) -> &'static str {
    match context {
        InstallContext::Standalone { .. } => "standalone",
        InstallContext::Npm => "npm",
        InstallContext::Bun => "bun",
        InstallContext::Brew => "brew",
        InstallContext::Other => "local build",
    }
}

fn search_provider(context: &InstallContext) -> &'static str {
    match context {
        InstallContext::Standalone {
            resources_dir: Some(resources_dir),
            ..
        } if context.rg_command().starts_with(resources_dir) => "bundled",
        _ => "system",
    }
}

fn build_commit() -> &'static str {
    option_env!("CODEX_BUILD_COMMIT")
        .or(option_env!("GIT_COMMIT"))
        .unwrap_or("unknown")
}
