//! Confirmation for copying the invoking CLI package into a daemon installation.

use std::io::IsTerminal;

use anyhow::Result;
use codex_app_server_daemon::InstallRequest;

pub(crate) fn confirm_install(request: &InstallRequest, yes: bool) -> Result<bool> {
    eprintln!("{}", describe_install(request));
    if yes {
        return Ok(true);
    }
    anyhow::ensure!(
        std::io::stdin().is_terminal() && std::io::stderr().is_terminal(),
        "daemon installation requires confirmation; rerun with --yes to copy this CLI package"
    );
    let confirmed = crate::confirm("Copy this package? [y/N]: ")?;
    if !confirmed {
        eprintln!("Daemon installation cancelled.");
    }
    Ok(confirmed)
}

fn describe_install(request: &InstallRequest) -> String {
    let mut message = format!(
        "Replace installed daemon version {} with CLI version {} from {}.\nThe daemon package will be installed in {}.\nThe selected package will be pinned. Run `codex app-server daemon update` to return to production updates.",
        request.installed_version.as_deref().unwrap_or("unknown"),
        request.version,
        request.source.display(),
        request.destination.display()
    );
    if request.restart_required {
        message.push_str(
            "\nThe running daemon will restart; active or queued work may be interrupted.",
        );
    }
    message
}

#[cfg(test)]
#[path = "daemon_install_tests.rs"]
mod tests;
