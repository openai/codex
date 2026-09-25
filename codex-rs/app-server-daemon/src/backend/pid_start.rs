//! Detached process launch and PID publication. Hold the reservation lock until
//! the record is published, and on Windows until an updater acknowledges startup.

use super::PidBackend;
#[cfg(windows)]
use super::PidCommandKind;
use super::PidFileState;
use super::PidRecord;
use super::read_process_start_time;
use crate::managed_install::executable_identity;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use std::process::Stdio;
use tokio::fs;
use tokio::process::Command;

impl PidBackend {
    pub(super) async fn start_inner(&self, replacement: Option<PidRecord>) -> Result<Option<u32>> {
        #[cfg(windows)]
        crate::backend::windows::ensure_not_elevated()?;
        if let Some(parent) = self.pid_file.parent() {
            codex_uds::prepare_private_socket_directory(parent)
                .await
                .with_context(|| format!("failed to create pid directory {}", parent.display()))?;
        }
        let reservation_lock = self.acquire_reservation_lock().await?;
        loop {
            match fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&self.pid_file)
                .await
            {
                Ok(pid_file) => {
                    drop(pid_file);
                    break;
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    match self.read_pid_file_state_with_lock_held().await? {
                        PidFileState::Missing => continue,
                        PidFileState::Running(record) => {
                            if replacement.as_ref() == Some(&record) {
                                break;
                            }
                            if replacement.is_some() {
                                if self.record_is_active(&record).await? {
                                    bail!("updater ownership changed before handoff");
                                }
                                // A failed rollback may leave the dead successor's record.
                                break;
                            }
                            if self.record_is_active(&record).await? {
                                return Ok(None);
                            }
                            let _ = fs::remove_file(&self.pid_file).await;
                            continue;
                        }
                        PidFileState::Starting => {
                            unreachable!("lock holder cannot observe starting")
                        }
                    }
                }
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("failed to reserve pid file {}", self.pid_file.display())
                    });
                }
            }
        }
        // Pin the selected release across installer symlink/junction retargeting.
        let codex_bin = fs::canonicalize(&self.codex_bin)
            .await
            .unwrap_or_else(|_| self.codex_bin.clone());
        let launched_identity =
            if matches!(self.command_kind, super::PidCommandKind::AppServer { .. }) {
                executable_identity(&codex_bin).await.ok()
            } else {
                None
            };
        let mut command = Command::new(&codex_bin);
        let stderr_log = match self.open_stderr_log().await {
            Ok(stderr_log) => stderr_log,
            Err(err) => {
                if replacement.is_none() {
                    let _ = fs::remove_file(&self.pid_file).await;
                }
                return Err(err);
            }
        };
        command
            // Handoff suppression belongs to the foreground CLI, not its long-lived children.
            .env_remove(crate::telemetry::HANDOFF_ENV)
            .args(self.command_args().iter().map(std::borrow::Cow::as_ref))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr_log.into_std().await));
        // Older or pinned managed binaries may predate this optional startup flag.
        let managed_app_server =
            matches!(self.command_kind, super::PidCommandKind::AppServer { .. });
        if managed_app_server
            && matches!(
                tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    Command::new(&codex_bin)
                        .args(["app-server", "--managed-daemon", "--help"])
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .kill_on_drop(true)
                        .status(),
                ).await,
                Ok(Ok(status)) if status.success()
            )
        {
            command.arg("--managed-daemon");
        } else if managed_app_server {
            let codex_home = self
                .pid_file
                .parent()
                .and_then(std::path::Path::parent)
                .context("daemon pid path has no Codex home")?;
            let recovery_file = codex_app_server_transport::daemon_recovery_file_path(codex_home);
            match fs::remove_file(&recovery_file).await {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    tracing::warn!(path = %recovery_file.display(), %err, "failed to clear daemon recovery state before legacy launch");
                }
            }
        }
        if let Some((key, value)) = self.command_env() {
            command.env(key, value);
        }

        #[cfg(unix)]
        {
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }

        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Threading::CREATE_BREAKAWAY_FROM_JOB;
            use windows_sys::Win32::System::Threading::DETACHED_PROCESS;
            // Preserve process-scoped paths before changing cwd; CA names match CUSTOM_CA_ENV_KEYS.
            for name in [
                "CODEX_HOME",
                "CODEX_SQLITE_HOME",
                "CODEX_CA_CERTIFICATE",
                "SSL_CERT_FILE",
                "REQUESTS_CA_BUNDLE",
                "CURL_CA_BUNDLE",
                "NODE_EXTRA_CA_CERTS",
                "GIT_SSL_CAINFO",
                "CARGO_HTTP_CAINFO",
                "PIP_CERT",
                "BUNDLE_SSL_CA_CERT",
                "npm_config_cafile",
                "AWS_CONFIG_FILE",
                "AWS_SHARED_CREDENTIALS_FILE",
                "AWS_WEB_IDENTITY_TOKEN_FILE",
                "AWS_CONTAINER_AUTHORIZATION_TOKEN_FILE",
            ] {
                let Some(mut value) = std::env::var_os(name) else {
                    continue;
                };
                if matches!(name, "CODEX_SQLITE_HOME" | "npm_config_cafile") {
                    value = value.to_str().unwrap_or_default().trim().into();
                }
                // These consumers expand `~` independently of the working directory.
                let expands_home = if name == "npm_config_cafile" {
                    value
                        .to_str()
                        .is_some_and(|path| path.starts_with("~/") || path.starts_with("~\\"))
                } else {
                    matches!(
                        name,
                        "CODEX_SQLITE_HOME" | "AWS_CONFIG_FILE" | "AWS_SHARED_CREDENTIALS_FILE"
                    ) && std::path::Path::new(&value).starts_with("~")
                };
                if value.is_empty() || expands_home {
                    continue;
                }
                command.env(name, std::path::absolute(value)?);
            }
            if let Some(value) = std::env::var_os("SSL_CERT_DIR") {
                let paths = std::env::split_paths(&value)
                    .filter(|path| !path.as_os_str().is_empty())
                    .map(std::path::absolute)
                    .collect::<std::io::Result<Vec<_>>>()?;
                command.env("SSL_CERT_DIR", std::env::join_paths(paths)?);
            }
            // A Windows process pins its working directory for its lifetime.
            // Keep both managed children out of the launching project's directory.
            command.current_dir(
                self.pid_file
                    .parent()
                    .context("daemon pid path has no parent")?,
            );
            // Never retry inside the parent's Job Object: that would report a
            // successful launch that dies when the terminal/SSH session closes.
            command.creation_flags(DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB);
            if matches!(self.command_kind, PidCommandKind::UpdateLoop { .. }) {
                match fs::remove_file(self.pid_file.with_extension("ready")).await {
                    Ok(()) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => return Err(err).context("failed to clear updater readiness"),
                }
            }
            match self.command_kind {
                PidCommandKind::AppServer { .. } => {
                    command.env(codex_app_server_transport::DAEMON_SHUTDOWN_SOCKET_ENV, "1");
                }
                PidCommandKind::UpdateLoop { .. } => {
                    let shutdown_file = self.pid_file.with_extension("shutdown");
                    match fs::remove_file(&shutdown_file).await {
                        Ok(()) => {}
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                        Err(err) => {
                            return Err(err).context("failed to clear updater shutdown request");
                        }
                    }
                    command.env(
                        codex_app_server_transport::DAEMON_SHUTDOWN_FILE_ENV,
                        shutdown_file,
                    );
                }
            }
        }

        #[cfg(windows)]
        let child = super::super::windows::spawn_without_inheriting_stdio(&mut command);
        #[cfg(not(windows))]
        let child = command.spawn().map_err(anyhow::Error::from);
        let child = match child {
            Ok(child) => child,
            Err(err) => {
                if replacement.is_none() {
                    let _ = fs::remove_file(&self.pid_file).await;
                }
                return Err(err).with_context(|| {
                    let job_hint = if cfg!(windows) {
                        " (the Windows host job must allow breakaway)"
                    } else {
                        ""
                    };
                    format!(
                        "failed to spawn detached app-server process using {}{job_hint}",
                        self.codex_bin.display()
                    )
                });
            }
        };
        let pid = child
            .id()
            .context("spawned app-server process has no pid")?;
        let record = match async {
            let process_start_time = read_process_start_time(pid).await?;
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            let process_identity = super::identity::read_process_details(pid)
                .await
                .ok()
                .map(|(_, identity)| identity);
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            let process_identity = None;
            anyhow::Ok(PidRecord {
                pid,
                process_start_time,
                process_identity,
                executable_identity: launched_identity,
            })
        }
        .await
        {
            Ok(record) => record,
            Err(err) => {
                let _ = self.terminate_process(pid);
                let mut context =
                    format!("failed to record pid-managed app-server process {pid} startup");
                super::super::append_stderr_log_tail_context(&self.pid_file, &mut context).await;
                if replacement.is_none() {
                    let _ = fs::remove_file(&self.pid_file).await;
                }
                return Err(err).context(context);
            }
        };
        let contents = serde_json::to_vec(&record).context("failed to serialize pid record")?;
        let temp_pid_file = self.pid_file.with_extension("pid.tmp");
        if let Err(err) = fs::write(&temp_pid_file, &contents).await {
            let _ = self.terminate_process(pid);
            if replacement.is_none() {
                let _ = fs::remove_file(&self.pid_file).await;
            }
            return Err(err).with_context(|| {
                format!("failed to write pid temp file {}", temp_pid_file.display())
            });
        }
        if let Err(err) = fs::rename(&temp_pid_file, &self.pid_file).await {
            let _ = self.terminate_process(pid);
            let _ = fs::remove_file(&temp_pid_file).await;
            if replacement.is_none() {
                let _ = fs::remove_file(&self.pid_file).await;
            }
            return Err(err).with_context(|| {
                format!("failed to publish pid file {}", self.pid_file.display())
            });
        }
        #[cfg(windows)]
        if matches!(self.command_kind, PidCommandKind::UpdateLoop { .. }) {
            self.finish_updater_start(&record, replacement.as_ref())
                .await?;
        }
        drop(reservation_lock);
        Ok(Some(pid))
    }
}
