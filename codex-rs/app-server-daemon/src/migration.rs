//! Moves a legacy daemon to its dedicated package only during an explicit update.
//! Preparation leaves the legacy selection intact; publishing current is the cutover.

use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use futures::FutureExt;

use super::InstallerHttp;
use super::InstallerMode;
use super::fetch_installer_script;
use super::run_installer_script;
use super::selected_release;
use crate::Daemon;
use crate::UpdateOutput;
use crate::UpdateStatus;
use crate::backend;
use crate::client;
use crate::install_lock::acquire_install_lock;
use crate::managed_install;

pub(super) async fn run(http: &impl InstallerHttp, legacy: &Daemon) -> Result<UpdateOutput> {
    let _operation_lock = legacy.acquire_operation_lock().await?;
    let previous = selected_release(legacy)?;
    anyhow::ensure!(
        previous.0.ends_with("standalone") && super::manual_update::supported(legacy)?,
        "daemon selection changed; retry the update"
    );
    let home = previous
        .0
        .parent()
        .and_then(Path::parent)
        .context("package root has no Codex home")?;
    let root = home.join("packages/app-server-daemon");
    let settings = legacy.load_settings().await?;
    let running = legacy.running_backend_instance(&settings).await?;
    if running.is_none() && client::probe(&legacy.socket_path).await.is_ok() {
        return super::manual_update::unsupported(legacy).await;
    }
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let cancellation = async {
        #[cfg(unix)]
        tokio::select! {
            _ = terminate.recv() => {},
            _ = tokio::signal::ctrl_c() => {},
        }
        #[cfg(windows)]
        let _ = tokio::signal::ctrl_c().await;
        Some(())
    };
    let mut cancellation = std::pin::pin!(cancellation);

    let script = fetch_installer_script(http).await?;
    anyhow::ensure!(
        script
            .windows(b"CODEX_INSTALL_DEFER_SELECTION".len())
            .any(|window| window == b"CODEX_INSTALL_DEFER_SELECTION"),
        "the published installer does not support daemon migration yet; the legacy installation was left unchanged"
    );
    eprintln!("Preparing the daemon update in {}...", root.display());
    #[cfg(unix)]
    {
        if matches!(
            run_installer_script(
                &script,
                InstallerMode::Migration,
                &root,
                cancellation.as_mut()
            )
            .await?,
            super::UpdateLoopControl::Stop
        ) {
            return Err(std::io::Error::from(std::io::ErrorKind::Interrupted).into());
        }
    }
    #[cfg(windows)]
    tokio::select! {
        result = run_installer_script(&script, InstallerMode::Migration, &root) => { result?; },
        _ = &mut cancellation => return Err(std::io::Error::from(std::io::ErrorKind::Interrupted).into()),
    }

    let _install_lock = acquire_install_lock(&root).await?;
    let pending = root.join(".migration-current");
    let release = pending
        .canonicalize()
        .context("installer did not prepare a daemon package")?;
    let name = release
        .file_name()
        .context("prepared release has no name")?;
    anyhow::ensure!(
        release.parent() == Some(root.join("releases").canonicalize()?.as_path())
            && std::fs::read_to_string(root.join("auto-update-version"))? == name.to_string_lossy(),
        "installer did not prepare a latest-channel daemon package"
    );
    let entrypoint = if cfg!(windows) {
        "bin/codex.exe"
    } else {
        "bin/codex"
    };
    let binary = release.join(entrypoint);
    let version = managed_install::managed_codex_version(&binary).await?;
    anyhow::ensure!(
        name.to_string_lossy().starts_with(&format!("{version}-")),
        "prepared daemon version does not match its release"
    );
    // An older production updater understands only the legacy package/PID paths.
    anyhow::ensure!(
        managed_install::supports_daemon_command(
            &binary,
            &["pid-update-loop", "--check-package-ownership"]
        )
        .await,
        "the latest production release does not support daemon migration yet; the legacy daemon was left running"
    );
    #[cfg(windows)]
    backend::windows::ensure_detached_launch(&binary)?;
    anyhow::ensure!(
        selected_release(legacy)? == previous,
        "daemon selection changed; retry the update"
    );
    if cancellation.as_mut().now_or_never().is_some() {
        return Err(std::io::Error::from(std::io::ErrorKind::Interrupted).into());
    }
    let cutover = async {
        backend::pid_update_loop_backend(legacy.backend_paths(&settings))
            .stop()
            .await?;
        anyhow::ensure!(
            selected_release(legacy)? == previous,
            "daemon selection changed; retry the update"
        );
        if let Some(running) = &running {
            crate::thread_recovery::discard_pending(legacy)?;
            running
                .stop_with_grace(settings.shutdown_grace_seconds)
                .await?;
        }
        std::fs::rename(&pending, root.join("current"))
            .context("failed to select the new daemon package; retry the update")
    }
    .await;
    if let Err(error) = cutover {
        if let Err(restore_error) = async {
            legacy
                .current_installation()?
                .ensure_managed_updater(&settings)
                .await
        }
        .await
        {
            eprintln!(
                "warning: failed to restore the legacy updater after migration failed: {restore_error:#}"
            );
        }
        return Err(error);
    }
    let selected = Daemon {
        pid_file: legacy.pid_file.with_file_name(crate::DAEMON_PID_FILE_NAME),
        update_pid_file: legacy
            .update_pid_file
            .with_file_name(crate::DAEMON_UPDATE_PID_FILE_NAME),
        managed_codex_bin: root.join("current").join(entrypoint),
        ..legacy.clone()
    };
    let running_version = if running.is_some() {
        selected.start_managed_backend(&settings).await.context(
            "daemon migrated but could not start; retry with `codex app-server daemon start`",
        )?;
        Some(
            selected
                .wait_until_ready()
                .await
                .context(
                    "daemon migrated but is not ready; retry with `codex app-server daemon start`",
                )?
                .app_server_version,
        )
    } else {
        None
    };
    if let Err(error) = selected.ensure_managed_updater(&settings).await {
        eprintln!("warning: daemon migrated but its updater could not start: {error:#}");
    }
    Ok(UpdateOutput {
        status: UpdateStatus::Updated,
        installed_version: Some(version),
        running_version,
        managed_codex_path: selected.managed_codex_bin,
        message: "The daemon was updated and moved to its dedicated package. The legacy CLI package was left unchanged.".to_string(),
    })
}
