use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use tokio::fs;
use tokio::process::Command;

const APP_SERVER_UNIT_NAME: &str = "codex-app-server.service";
const UPDATE_UNIT_NAME: &str = "codex-app-server-update.service";
const UPDATE_TIMER_NAME: &str = "codex-app-server-update.timer";
const RESTART_GRACE_PERIOD_SECONDS: u32 = 60;
const RESTART_TIMEOUT_SECONDS: u32 = 70;

#[derive(Debug)]
pub(crate) struct SystemdBackend {
    codex_bin: PathBuf,
    remote_control_enabled: bool,
}

impl SystemdBackend {
    pub(crate) fn new(codex_bin: PathBuf, remote_control_enabled: bool) -> Self {
        Self {
            codex_bin,
            remote_control_enabled,
        }
    }

    pub(crate) async fn is_available() -> bool {
        systemctl_user_available().await && command_succeeds("systemd-run", &["--version"]).await
    }

    pub(crate) async fn is_running(&self) -> Result<bool> {
        let output = match Command::new("systemctl")
            .args(["--user", "is-active", APP_SERVER_UNIT_NAME])
            .output()
            .await
        {
            Ok(output) => output,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(err) => {
                return Err(err).context("failed to inspect user systemd app-server unit");
            }
        };
        Ok(output.status.success() && !is_chroot_stub_response(&output))
    }

    pub(crate) async fn start(&self) -> Result<Option<u32>> {
        if self.persistent_service_exists() {
            run_systemctl(&["--user", "start", APP_SERVER_UNIT_NAME]).await?;
            return Ok(None);
        }

        let output = Command::new("systemd-run")
            .args([
                "--user",
                "--unit",
                APP_SERVER_UNIT_NAME,
                "--collect",
                "--property",
                "Restart=on-failure",
            ])
            .arg(&self.codex_bin)
            .args(self.app_server_args())
            .output()
            .await
            .context("failed to invoke systemd-run for app-server")?;

        if output.status.success() {
            return Ok(None);
        }

        bail!(
            "failed to start app-server through user systemd: {}",
            stderr_or_status(&output)
        )
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        run_systemctl(&["--user", "stop", APP_SERVER_UNIT_NAME]).await
    }

    pub(crate) async fn bootstrap(
        codex_home: &Path,
        managed_codex_bin: &Path,
        remote_control_enabled: bool,
    ) -> Result<()> {
        let unit_dir = user_unit_dir()?;
        fs::create_dir_all(&unit_dir)
            .await
            .with_context(|| format!("failed to create user systemd dir {}", unit_dir.display()))?;

        let service =
            render_app_server_service(codex_home, managed_codex_bin, remote_control_enabled);
        let update_service = render_update_service(codex_home);
        fs::write(unit_dir.join(APP_SERVER_UNIT_NAME), service)
            .await
            .with_context(|| format!("failed to write {APP_SERVER_UNIT_NAME}"))?;
        fs::write(unit_dir.join(UPDATE_UNIT_NAME), update_service)
            .await
            .with_context(|| format!("failed to write {UPDATE_UNIT_NAME}"))?;
        fs::write(unit_dir.join(UPDATE_TIMER_NAME), render_update_timer())
            .await
            .with_context(|| format!("failed to write {UPDATE_TIMER_NAME}"))?;

        run_systemctl(&["--user", "daemon-reload"]).await?;
        run_systemctl(&["--user", "enable", "--now", UPDATE_TIMER_NAME]).await?;
        run_systemctl(&["--user", "enable", APP_SERVER_UNIT_NAME]).await?;
        run_systemctl(&["--user", "restart", APP_SERVER_UNIT_NAME]).await?;
        Ok(())
    }

    fn app_server_args(&self) -> Vec<&'static str> {
        if self.remote_control_enabled {
            vec![
                "--enable",
                "remote_control",
                "app-server",
                "--listen",
                "unix://",
            ]
        } else {
            vec!["app-server", "--listen", "unix://"]
        }
    }

    fn persistent_service_exists(&self) -> bool {
        user_unit_dir()
            .ok()
            .is_some_and(|unit_dir| unit_dir.join(APP_SERVER_UNIT_NAME).is_file())
    }
}

fn render_app_server_service(
    codex_home: &Path,
    managed_codex_bin: &Path,
    remote_control_enabled: bool,
) -> String {
    let remote_control_args = if remote_control_enabled {
        " --enable remote_control"
    } else {
        ""
    };
    let exec_stop = quote_systemd_arg(render_exec_stop_command());
    format!(
        "[Unit]\nDescription=Codex app-server\n\n[Service]\nType=simple\nEnvironment={}\nExecStart={}{} app-server --listen unix://\nExecReload=/bin/kill -TERM $MAINPID\nExecStop=/bin/sh -lc {}\nRestart=always\nTimeoutStopSec={}s\n\n[Install]\nWantedBy=default.target\n",
        quote_systemd_env("CODEX_HOME", codex_home),
        quote_systemd_path(managed_codex_bin),
        remote_control_args,
        exec_stop,
        RESTART_TIMEOUT_SECONDS,
    )
}

fn render_update_service(codex_home: &Path) -> String {
    format!(
        "[Unit]\nDescription=Update standalone Codex install\n\n[Service]\nType=oneshot\nEnvironment={}\nExecStart=/bin/sh -lc {}\nExecStartPost=systemctl --user reload {}\n",
        quote_systemd_env("CODEX_HOME", codex_home),
        quote_systemd_arg("curl -fsSL https://chatgpt.com/codex/install.sh | sh"),
        APP_SERVER_UNIT_NAME,
    )
}

fn render_update_timer() -> &'static str {
    "[Unit]\nDescription=Periodically update standalone Codex install\n\n[Timer]\nOnBootSec=5m\nOnUnitActiveSec=1h\nRandomizedDelaySec=15m\nPersistent=true\nUnit=codex-app-server-update.service\n\n[Install]\nWantedBy=timers.target\n"
}

fn quote_systemd_env(key: &str, value: &Path) -> String {
    let value = value.to_string_lossy();
    quote_systemd_arg(format!("{key}={value}"))
}

fn quote_systemd_path(value: &Path) -> String {
    quote_systemd_arg(value.to_string_lossy())
}

fn quote_systemd_arg(value: impl AsRef<str>) -> String {
    let escaped = value.as_ref().replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn render_exec_stop_command() -> String {
    format!(
        "pid=$MAINPID; kill -TERM \"$pid\"; i=0; while kill -0 \"$pid\" 2>/dev/null; do if [ \"$i\" -ge {RESTART_GRACE_PERIOD_SECONDS} ]; then kill -TERM \"$pid\"; fi; i=$((i + 1)); sleep 1; done"
    )
}

fn user_unit_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path).join("systemd").join("user"));
    }

    let home = home_dir().context("failed to resolve home directory for user systemd units")?;
    Ok(home.join(".config").join("systemd").join("user"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

async fn run_systemctl(args: &[&str]) -> Result<()> {
    let output = Command::new("systemctl")
        .args(args)
        .output()
        .await
        .with_context(|| format!("failed to invoke systemctl {}", args.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }

    bail!(
        "systemctl {} failed: {}",
        args.join(" "),
        stderr_or_status(&output)
    )
}

async fn command_succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .await
        .is_ok_and(|output| output.status.success())
}

async fn systemctl_user_available() -> bool {
    Command::new("systemctl")
        .args(["--user", "show-environment"])
        .output()
        .await
        .is_ok_and(|output| output.status.success() && !is_chroot_stub_response(&output))
}

fn is_chroot_stub_response(output: &std::process::Output) -> bool {
    String::from_utf8_lossy(&output.stderr).contains("Running in chroot, ignoring command")
}

fn stderr_or_status(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        output.status.to_string()
    } else {
        stderr
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::ExitStatus;
    use std::process::Output;

    use pretty_assertions::assert_eq;

    use super::is_chroot_stub_response;
    use super::render_app_server_service;
    use super::render_exec_stop_command;
    use super::render_update_service;
    use super::render_update_timer;

    #[test]
    fn app_server_service_enables_remote_control_and_unbounded_drain() {
        assert_eq!(
            render_app_server_service(
                Path::new("/home/codex/.codex"),
                Path::new("/home/codex/.codex/packages/standalone/current/codex"),
                true,
            ),
            "[Unit]\nDescription=Codex app-server\n\n[Service]\nType=simple\nEnvironment=\"CODEX_HOME=/home/codex/.codex\"\nExecStart=\"/home/codex/.codex/packages/standalone/current/codex\" --enable remote_control app-server --listen unix://\nExecReload=/bin/kill -TERM $MAINPID\nExecStop=/bin/sh -lc \"pid=$MAINPID; kill -TERM \\\"$pid\\\"; i=0; while kill -0 \\\"$pid\\\" 2>/dev/null; do if [ \\\"$i\\\" -ge 60 ]; then kill -TERM \\\"$pid\\\"; fi; i=$((i + 1)); sleep 1; done\"\nRestart=always\nTimeoutStopSec=70s\n\n[Install]\nWantedBy=default.target\n"
        );
    }

    #[test]
    fn update_service_reinstalls_then_restarts_app_server() {
        assert_eq!(
            render_update_service(Path::new("/home/codex/.codex")),
            "[Unit]\nDescription=Update standalone Codex install\n\n[Service]\nType=oneshot\nEnvironment=\"CODEX_HOME=/home/codex/.codex\"\nExecStart=/bin/sh -lc \"curl -fsSL https://chatgpt.com/codex/install.sh | sh\"\nExecStartPost=systemctl --user reload codex-app-server.service\n"
        );
    }

    #[test]
    fn exec_stop_waits_then_forces_restart() {
        assert_eq!(
            render_exec_stop_command(),
            "pid=$MAINPID; kill -TERM \"$pid\"; i=0; while kill -0 \"$pid\" 2>/dev/null; do if [ \"$i\" -ge 60 ]; then kill -TERM \"$pid\"; fi; i=$((i + 1)); sleep 1; done"
        );
    }

    #[test]
    fn update_timer_uses_jitter_and_persistence() {
        assert_eq!(
            render_update_timer(),
            "[Unit]\nDescription=Periodically update standalone Codex install\n\n[Timer]\nOnBootSec=5m\nOnUnitActiveSec=1h\nRandomizedDelaySec=15m\nPersistent=true\nUnit=codex-app-server-update.service\n\n[Install]\nWantedBy=timers.target\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn chroot_systemctl_stub_response_is_rejected() {
        use std::os::unix::process::ExitStatusExt;

        assert!(is_chroot_stub_response(&Output {
            status: ExitStatus::from_raw(0),
            stdout: Vec::new(),
            stderr: b"Running in chroot, ignoring command 'show-environment'\n".to_vec(),
        }));
    }
}
