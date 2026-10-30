#![cfg(test)]

use super::*;
use codex_tui::DaemonUpdateSource;
use pretty_assertions::assert_eq;
use std::os::unix::fs::PermissionsExt;

#[test]
fn package_update_runs_outside_the_project() -> anyhow::Result<()> {
    const CHILD: &str = "CODEX_TEST_PACKAGE_UPDATE_CHILD";
    if std::env::var_os(CHILD).is_some() {
        return run_update_action(UpdateAction::PnpmGlobalLatest, /*cli_executable*/ None);
    }

    let project = tempfile::tempdir()?;
    let bin = tempfile::tempdir()?;
    codex_utils_cargo_bin::write_executable(
        &bin.path().join("pnpm"),
        "#!/bin/sh\npwd -P > \"$0.cwd\"\n",
    )?;
    assert_cmd::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "daemon_update_tests::package_update_runs_outside_the_project",
        ])
        .current_dir(project.path())
        .env("TMPDIR", project.path())
        .env("PATH", bin.path())
        .env(CHILD, "1")
        .assert()
        .success();

    let cwd = std::fs::read_to_string(bin.path().join("pnpm.cwd"))?;
    assert!(!std::path::Path::new(cwd.trim_end()).starts_with(project.path().canonicalize()?));
    Ok(())
}

#[test]
fn daemon_handoff_uses_selected_executable_and_propagates_failure() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let executable = dir.path().join("launching CLI");
    let receipt = dir.path().join("launching CLI.args");
    codex_utils_cargo_bin::write_executable(
        &executable,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\n",
    )?;
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(/*mode*/ 0o700))?;
    for (source, expected) in [
        (
            DaemonUpdateSource::PublicStable,
            "app-server\ndaemon\nupdate\n",
        ),
        (
            DaemonUpdateSource::ThisCli,
            "app-server\ndaemon\nupdate\n--from-cli\n--yes\n",
        ),
    ] {
        run_update_action(UpdateAction::Daemon(source), Some(&executable))?;
        assert_eq!(std::fs::read_to_string(&receipt)?, expected);
    }
    codex_utils_cargo_bin::write_executable(&executable, "#!/bin/sh\nexit 7\n")?;
    let error = run_update_action(
        UpdateAction::Daemon(DaemonUpdateSource::ThisCli),
        Some(&executable),
    )
    .unwrap_err();
    assert!(error.to_string().contains("Daemon update failed"));
    Ok(())
}
