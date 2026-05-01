use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use sha2::Digest;
use sha2::Sha256;
use tokio::fs;
use tokio::process::Command;

const RESTART_TIMEOUT_SECONDS: u32 = 60;

#[derive(Debug)]
pub(crate) struct SystemdBackend {
    codex_home: PathBuf,
    codex_bin: PathBuf,
    remote_control_enabled: bool,
    unit_names: SystemdUnitNames,
}

impl SystemdBackend {
    pub(crate) fn new(
        codex_home: PathBuf,
        codex_bin: PathBuf,
        remote_control_enabled: bool,
    ) -> Self {
        let unit_names = SystemdUnitNames::for_codex_home(&codex_home);
        Self {
            codex_home,
            codex_bin,
            remote_control_enabled,
            unit_names,
        }
    }

    pub(crate) async fn is_available() -> bool {
        systemctl_user_available().await && command_succeeds("systemd-run", &["--version"]).await
    }

    pub(crate) async fn is_starting_or_running(&self) -> Result<bool> {
        let output = match Command::new("systemctl")
            .args([
                "--user",
                "show",
                "--property",
                "ActiveState",
                "--value",
                &self.unit_names.app_server,
            ])
            .output()
            .await
        {
            Ok(output) => output,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(err) => {
                return Err(err).context("failed to inspect user systemd app-server unit");
            }
        };
        if !output.status.success() || is_chroot_stub_response(&output) {
            return Ok(false);
        }

        Ok(matches!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "active" | "activating" | "deactivating" | "reloading"
        ))
    }

    pub(crate) async fn start(&self) -> Result<Option<u32>> {
        if self.persistent_service_exists() {
            run_systemctl(&["--user", "start", &self.unit_names.app_server]).await?;
            return Ok(None);
        }

        let output = Command::new("systemd-run")
            .args([
                "--user",
                "--unit",
                &self.unit_names.app_server,
                "--collect",
                "--setenv",
                &format!("CODEX_HOME={}", self.codex_home.display()),
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
        run_systemctl(&["--user", "stop", &self.unit_names.app_server]).await
    }

    pub(crate) async fn bootstrap(
        codex_home: &Path,
        managed_codex_bin: &Path,
        remote_control_enabled: bool,
    ) -> Result<()> {
        let unit_dir = user_unit_dir()?;
        let unit_names = SystemdUnitNames::for_codex_home(codex_home);
        fs::create_dir_all(&unit_dir)
            .await
            .with_context(|| format!("failed to create user systemd dir {}", unit_dir.display()))?;

        let service =
            render_app_server_service(codex_home, managed_codex_bin, remote_control_enabled);
        let update_service = render_update_service(codex_home, &unit_names);
        fs::write(unit_dir.join(&unit_names.app_server), service)
            .await
            .with_context(|| format!("failed to write {}", unit_names.app_server))?;
        fs::write(unit_dir.join(&unit_names.update_service), update_service)
            .await
            .with_context(|| format!("failed to write {}", unit_names.update_service))?;
        fs::write(
            unit_dir.join(&unit_names.update_timer),
            render_update_timer(&unit_names),
        )
        .await
        .with_context(|| format!("failed to write {}", unit_names.update_timer))?;

        run_systemctl(&["--user", "daemon-reload"]).await?;
        run_systemctl(&["--user", "enable", "--now", &unit_names.update_timer]).await?;
        run_systemctl(&["--user", "enable", &unit_names.app_server]).await?;
        run_systemctl(&["--user", "restart", &unit_names.app_server]).await?;
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
            .is_some_and(|unit_dir| unit_dir.join(&self.unit_names.app_server).is_file())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SystemdUnitNames {
    app_server: String,
    update_service: String,
    update_timer: String,
}

impl SystemdUnitNames {
    fn for_codex_home(codex_home: &Path) -> Self {
        let digest = Sha256::digest(codex_home.as_os_str().as_encoded_bytes());
        let suffix = format!("{digest:x}");
        let suffix = &suffix[..16];
        Self {
            app_server: format!("codex-app-server-{suffix}.service"),
            update_service: format!("codex-app-server-update-{suffix}.service"),
            update_timer: format!("codex-app-server-update-{suffix}.timer"),
        }
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
    format!(
        "[Unit]\nDescription=Codex app-server\n\n[Service]\nType=simple\nEnvironment={}\nExecStart={}{} app-server --listen unix://\nExecReload=/bin/kill -HUP $MAINPID\nRestart=always\nTimeoutStopSec={}s\n\n[Install]\nWantedBy=default.target\n",
        quote_systemd_env("CODEX_HOME", codex_home),
        quote_systemd_path(managed_codex_bin),
        remote_control_args,
        RESTART_TIMEOUT_SECONDS,
    )
}

fn render_update_service(codex_home: &Path, unit_names: &SystemdUnitNames) -> String {
    format!(
        "[Unit]\nDescription=Update standalone Codex install\n\n[Service]\nType=oneshot\nEnvironment={}\nExecStart=/bin/sh -c {}\nExecStartPost=systemctl --user reload {}\n",
        quote_systemd_env("CODEX_HOME", codex_home),
        quote_systemd_arg(
            "tmp=$$(mktemp) && trap 'rm -f \"$$tmp\"' EXIT && curl -fsSL https://chatgpt.com/codex/install.sh -o \"$$tmp\" && sh \"$$tmp\"",
        ),
        unit_names.app_server,
    )
}

fn render_update_timer(unit_names: &SystemdUnitNames) -> String {
    format!(
        "[Unit]\nDescription=Periodically update standalone Codex install\n\n[Timer]\nOnActiveSec=5m\nOnUnitActiveSec=1h\nRandomizedDelaySec=15m\nPersistent=true\nUnit={}\n\n[Install]\nWantedBy=timers.target\n",
        unit_names.update_service
    )
}

fn quote_systemd_env(key: &str, value: &Path) -> String {
    let value = value.to_string_lossy();
    quote_systemd_arg(format!("{key}={value}"))
}

fn quote_systemd_path(value: &Path) -> String {
    quote_systemd_arg(value.to_string_lossy().replace('$', "$$"))
}

fn quote_systemd_arg(value: impl AsRef<str>) -> String {
    let escaped = value
        .as_ref()
        .replace('%', "%%")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\"{escaped}\"")
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

    use super::SystemdUnitNames;
    use super::is_chroot_stub_response;
    use super::quote_systemd_arg;
    use super::quote_systemd_path;
    use super::render_app_server_service;
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
            "[Unit]\nDescription=Codex app-server\n\n[Service]\nType=simple\nEnvironment=\"CODEX_HOME=/home/codex/.codex\"\nExecStart=\"/home/codex/.codex/packages/standalone/current/codex\" --enable remote_control app-server --listen unix://\nExecReload=/bin/kill -HUP $MAINPID\nRestart=always\nTimeoutStopSec=60s\n\n[Install]\nWantedBy=default.target\n"
        );
    }

    #[test]
    fn update_service_reinstalls_then_restarts_app_server() {
        let unit_names = test_unit_names();
        assert_eq!(
            render_update_service(Path::new("/home/codex/.codex"), &unit_names),
            "[Unit]\nDescription=Update standalone Codex install\n\n[Service]\nType=oneshot\nEnvironment=\"CODEX_HOME=/home/codex/.codex\"\nExecStart=/bin/sh -c \"tmp=$$(mktemp) && trap 'rm -f \\\"$$tmp\\\"' EXIT && curl -fsSL https://chatgpt.com/codex/install.sh -o \\\"$$tmp\\\" && sh \\\"$$tmp\\\"\"\nExecStartPost=systemctl --user reload codex-app-server-test.service\n"
        );
    }

    #[test]
    fn update_timer_uses_jitter_and_persistence() {
        let unit_names = test_unit_names();
        assert_eq!(
            render_update_timer(&unit_names),
            "[Unit]\nDescription=Periodically update standalone Codex install\n\n[Timer]\nOnActiveSec=5m\nOnUnitActiveSec=1h\nRandomizedDelaySec=15m\nPersistent=true\nUnit=codex-app-server-update-test.service\n\n[Install]\nWantedBy=timers.target\n"
        );
    }

    fn test_unit_names() -> SystemdUnitNames {
        SystemdUnitNames {
            app_server: "codex-app-server-test.service".to_string(),
            update_service: "codex-app-server-update-test.service".to_string(),
            update_timer: "codex-app-server-update-test.timer".to_string(),
        }
    }

    #[test]
    fn quote_systemd_arg_escapes_specifiers() {
        assert_eq!(quote_systemd_arg("/tmp/codex%h"), "\"/tmp/codex%%h\"");
    }

    #[test]
    fn quote_systemd_path_escapes_exec_start_variables() {
        assert_eq!(
            quote_systemd_path(Path::new("/tmp/codex$dev")),
            "\"/tmp/codex$$dev\""
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
