use super::*;
use codex_tui::DaemonUpdateSource;
use pretty_assertions::assert_eq;
use std::os::unix::fs::PermissionsExt;

#[test]
fn daemon_handoff_uses_selected_executable_and_propagates_failure() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let executable = dir.path().join("launching CLI");
    let receipt = dir.path().join("launching CLI.args");
    std::fs::write(
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
    std::fs::write(&executable, "#!/bin/sh\nexit 7\n")?;
    let error = run_update_action(
        UpdateAction::Daemon(DaemonUpdateSource::ThisCli),
        Some(&executable),
    )
    .unwrap_err();
    assert!(error.to_string().contains("Daemon update failed"));
    Ok(())
}
