use std::sync::Mutex;
#[cfg(unix)]
use std::time::Duration;

use pretty_assertions::assert_eq;
#[cfg(unix)]
use tempfile::TempDir;

use super::INSTALL_URL;
use super::InstallerHttp;
use super::InstallerResponse;
use super::fetch_installer_script;
#[cfg(unix)]
use super::manual_update::run as manual_update_once;
#[cfg(unix)]
use crate::Daemon;
#[cfg(unix)]
use crate::UpdateOutput;
#[cfg(unix)]
use crate::UpdateStatus;
#[cfg(unix)]
use crate::managed_install::executable_identity;
#[cfg(unix)]
use crate::managed_install::executable_identity_from_bytes;

#[tokio::test]
async fn installer_fetch_uses_exact_url_and_preserves_bytes() {
    let script = b"#!/bin/sh\nprintf 'update bytes'\n".to_vec();
    let http = FakeInstallerHttp::new(InstallerResponse::Success(script.clone()));

    assert_eq!(
        fetch_installer_script(&http)
            .await
            .expect("installer fetch should succeed"),
        script
    );
    assert_eq!(http.requested_urls(), vec![INSTALL_URL.to_string()]);
}

#[tokio::test]
async fn installer_fetch_rejects_non_success_status() {
    let http = FakeInstallerHttp::new(InstallerResponse::Unsuccessful { status: 503 });

    let error = fetch_installer_script(&http)
        .await
        .expect_err("non-success response should fail");

    assert!(error.to_string().contains("503"));
    assert_eq!(http.requested_urls(), vec![INSTALL_URL.to_string()]);
}

struct FakeInstallerHttp {
    response: InstallerResponse,
    requested_urls: Mutex<Vec<String>>,
}

#[cfg(unix)]
#[tokio::test]
async fn explicit_update_migrates_running_and_stopped_installations() {
    for running in [false, true] {
        let home = TempDir::new().unwrap();
        let (legacy, release) = manual_update_daemon(&home);
        let scheduled = FakeInstallerHttp::new(InstallerResponse::Success(
        b"# CODEX_INSTALL_IF_LATEST\ntest \"$CODEX_INSTALL_DEFER_SELECTION\" = 0 && test \"$CODEX_INSTALL_DAEMON_ONLY\" = 0\n".to_vec(),
    ));
        super::update_once(
            &scheduled,
            &legacy,
            &executable_identity(&legacy.managed_codex_bin)
                .await
                .unwrap(),
            &mut test_terminate(),
            super::UpdateTrigger::Scheduled,
        )
        .await
        .unwrap();
        assert_eq!(scheduled.requested_urls(), vec![INSTALL_URL.to_string()]);
        assert_eq!(
            crate::managed_install::package_root(home.path()),
            home.path().join("packages/standalone")
        );
        let settings = format!(
            r#"{{"updater":{{"autoUpdateEnabled":{running}}},"remoteControlEnabled":true}}"#
        );
        std::fs::write(&legacy.settings_file, &settings).unwrap();
        let daemon_settings = legacy.load_settings().await.unwrap();
        let old_backend = crate::backend::pid_backend(legacy.backend_paths(&daemon_settings));
        let old_updater =
            crate::backend::pid_update_loop_backend(legacy.backend_paths(&daemon_settings));
        let server = if running {
            old_backend.start().await.unwrap();
            old_updater.start().await.unwrap();
            Some(test_control_server(&legacy, home.path()).await)
        } else {
            None
        };
        let legacy_bin = std::fs::read(&legacy.managed_codex_bin).unwrap();
        let dedicated = home.path().join("packages/app-server-daemon");
        let old_installer = FakeInstallerHttp::new(InstallerResponse::Success(b"exit 99".to_vec()));
        assert!(
            super::migration::run(&old_installer, &legacy)
                .await
                .unwrap_err()
                .to_string()
                .contains("does not support daemon migration")
        );

        let script = format!(
            r#"# CODEX_INSTALL_DEFER_SELECTION
set -eu
test "$CODEX_INSTALL_DEFER_SELECTION" = 1
test "$CODEX_INSTALL_DAEMON_ONLY" = 1
test "$CODEX_INSTALL_IF_LATEST" = 0
root="$CODEX_HOME/packages/app-server-daemon"
mkdir -p "$root/releases/{release}/bin"
printf '#!/bin/sh\nif [ "$1" = --version ]; then echo codex 1.0.0; else exit 2; fi\n' > "$root/releases/{release}/bin/codex"
chmod +x "$root/releases/{release}/bin/codex"
ln -sfn 'releases/{release}' "$root/.migration-current"
printf '{release}' > "$root/auto-update-version"
"#
        );
        let http = FakeInstallerHttp::new(InstallerResponse::Success(script.as_bytes().to_vec()));
        let error = super::migration::run(&http, &legacy).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("production release does not support daemon migration")
        );
        assert!(!dedicated.join("current").exists());
        assert_eq!(
            crate::managed_install::package_root(home.path()),
            home.path().join("packages/standalone")
        );
        assert_eq!(
            (
                old_backend.is_starting_or_running().await.unwrap(),
                old_updater.is_starting_or_running().await.unwrap()
            ),
            (running, running)
        );
        let compatible = FakeInstallerHttp::new(InstallerResponse::Success(script.replace("else exit 2", "elif [ \"$4\" = --check-package-ownership ] || [ \"$4\" = --help ]; then exit 0; else exec sleep 30").into_bytes()));
        let output = super::migration::run(&compatible, &legacy).await.unwrap();
        assert_eq!(output, UpdateOutput {
        status: UpdateStatus::Updated,
        installed_version: Some("1.0.0".to_string()),
        running_version: (running).then(|| "1.0.0".to_string()),
        managed_codex_path: dedicated.join("current/bin/codex"),
        message: "The daemon was updated and moved to its dedicated package. The legacy CLI package was left unchanged.".to_string(),
    });
        assert_eq!(
            crate::managed_install::package_root(home.path()),
            dedicated.clone()
        );
        assert!(!dedicated.join(".migration-current").exists());
        assert_eq!(
            std::fs::read(&legacy.managed_codex_bin).unwrap(),
            legacy_bin
        );
        assert_eq!(
            std::fs::read_to_string(&legacy.settings_file).unwrap(),
            settings
        );
        let selected = Daemon {
            pid_file: legacy.pid_file.with_file_name(crate::DAEMON_PID_FILE_NAME),
            update_pid_file: legacy
                .update_pid_file
                .with_file_name(crate::DAEMON_UPDATE_PID_FILE_NAME),
            managed_codex_bin: dedicated.join("current/bin/codex"),
            ..legacy.clone()
        };
        let new_backend = crate::backend::pid_backend(selected.backend_paths(&daemon_settings));
        assert_eq!(new_backend.is_starting_or_running().await.unwrap(), running);
        assert!(!old_backend.is_starting_or_running().await.unwrap());
        assert!(!old_updater.is_starting_or_running().await.unwrap());
        let new_updater =
            crate::backend::pid_update_loop_backend(selected.backend_paths(&daemon_settings));
        assert_eq!(new_updater.is_starting_or_running().await.unwrap(), running);
        new_updater.stop().await.unwrap();
        new_backend.stop().await.unwrap();
        if let Some(server) = server {
            server.abort();
        }
    }
}

impl FakeInstallerHttp {
    fn new(response: InstallerResponse) -> Self {
        Self {
            response,
            requested_urls: Mutex::new(Vec::new()),
        }
    }

    fn requested_urls(&self) -> Vec<String> {
        self.requested_urls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl InstallerHttp for FakeInstallerHttp {
    async fn get(&self, url: &str) -> anyhow::Result<InstallerResponse> {
        self.requested_urls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(url.to_string());
        Ok(self.response.clone())
    }
}

#[cfg(unix)]
#[tokio::test]
async fn cancelling_installer_stops_children_and_releases_fallback_lock() {
    let home = tempfile::TempDir::new().expect("home");
    let ready = home.path().join("ready");
    let delayed = home.path().join("delayed");
    let lock = home.path().join("packages/standalone/install.lock.d");
    let script = format!(
        "mkdir -p '{lock}'\necho $$ > '{lock}/pid'\n(trap '' TERM; echo ready > '{ready}'; sleep 4; echo late > '{delayed}') &\nwait\n",
        lock = lock.display(),
        ready = ready.display(),
        delayed = delayed.display(),
    );
    let (cancel, cancelled) = tokio::sync::oneshot::channel();
    let ready_for_signal = ready.clone();
    let signal_sender = tokio::spawn(async move {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !ready_for_signal.exists() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "installer did not start"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        cancel.send(()).expect("cancel installer");
    });
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        super::run_installer_script(
            script.as_bytes(),
            super::InstallerMode::Update("0.150.0-test"),
            &home.path().join("packages/standalone"),
            async { cancelled.await.ok() },
        ),
    )
    .await
    .expect("installer cancellation timed out")
    .expect("installer cancellation failed");
    signal_sender.await.expect("signal sender");
    assert!(matches!(result, super::UpdateLoopControl::Stop));
    assert!(!lock.exists());
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(!delayed.exists());
}

#[cfg(unix)]
fn manual_update_daemon(home: &TempDir) -> (Daemon, String) {
    use std::os::unix::fs::PermissionsExt;

    let target = if cfg!(target_os = "macos") {
        format!("{}-apple-darwin", std::env::consts::ARCH)
    } else {
        format!("{}-unknown-linux-musl", std::env::consts::ARCH)
    };
    let release = format!("1.0.0-{target}");
    let standalone = home.path().join("packages/standalone");
    let bin = standalone.join("releases").join(&release).join("codex");
    std::fs::create_dir_all(bin.parent().expect("binary parent")).expect("release directory");
    std::fs::write(
        &bin,
        b"#!/bin/sh\nif [ \"$1\" = '--version' ]; then echo codex 1.0.0; else exec sleep 30; fi\n",
    )
    .expect("managed binary");
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755))
        .expect("executable binary");
    std::os::unix::fs::symlink(format!("releases/{release}"), standalone.join("current"))
        .expect("current release");
    std::fs::write(standalone.join("auto-update-version"), &release).expect("latest marker");
    let state = home.path().join("app-server-daemon");
    std::fs::create_dir(&state).unwrap();
    std::fs::write(state.join("app-server.stderr.log"), b"").unwrap();
    (
        Daemon {
            socket_path: home.path().join("app-server-control/server.sock"),
            pid_file: state.join("app-server.pid"),
            update_pid_file: state.join("app-server-updater.pid"),
            operation_lock_file: state.join("daemon.lock"),
            settings_file: state.join("settings.json"),
            managed_codex_bin: standalone.join("current/codex"),
        },
        release,
    )
}

#[cfg(unix)]
fn test_terminate() -> tokio::signal::unix::Signal {
    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("install test signal handler")
}

#[cfg(unix)]
#[tokio::test]
async fn manual_request_retries_after_updater_replacement() {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let home = TempDir::new().expect("home");
    let (daemon, _) = manual_update_daemon(&home);
    let socket_path = daemon.manual_update_socket_path();
    codex_uds::prepare_private_socket_directory(socket_path.parent().expect("socket parent"))
        .await
        .expect("socket directory");
    let mut listener = codex_uds::UnixListener::bind(&socket_path)
        .await
        .expect("old updater socket");
    let expected = UpdateOutput {
        status: UpdateStatus::NoUpdate,
        managed_codex_path: daemon.managed_codex_bin.clone(),
        installed_version: None,
        running_version: None,
        message: "already current".to_string(),
    };
    let reply = expected.clone();
    let server = tokio::spawn(async move {
        let mut old = listener.accept().await.expect("first connection");
        let mut request = [0; 7];
        old.read_exact(&mut request).await.expect("first request");
        drop(old);
        drop(listener);
        tokio::fs::remove_file(&socket_path)
            .await
            .expect("remove old socket");
        let mut successor = codex_uds::UnixListener::bind(&socket_path)
            .await
            .expect("successor socket");
        let mut connection = successor.accept().await.expect("retried connection");
        connection
            .read_exact(&mut request)
            .await
            .expect("retried request");
        connection
            .write_all(&serde_json::to_vec(&Ok::<_, String>(reply)).expect("serialize response"))
            .await
            .expect("send response");
    });
    assert_eq!(
        super::manual_update::request(&daemon)
            .await
            .expect("request survives handoff"),
        expected
    );
    server.await.expect("replacement task");
}

#[cfg(unix)]
#[tokio::test]
async fn manual_request_recovers_when_one_shot_updater_exits() {
    use tokio::io::AsyncReadExt;

    let home = TempDir::new().expect("home");
    let (daemon, _) = manual_update_daemon(&home);
    let socket_path = daemon.manual_update_socket_path();
    codex_uds::prepare_private_socket_directory(socket_path.parent().expect("socket parent"))
        .await
        .expect("socket directory");
    let mut listener = codex_uds::UnixListener::bind(&socket_path)
        .await
        .expect("one-shot updater socket");
    let server = tokio::spawn(async move {
        let mut connection = listener.accept().await.expect("request connection");
        let mut request = [0; 7];
        connection.read_exact(&mut request).await.expect("request");
        drop(connection);
        drop(listener);
        tokio::fs::remove_file(socket_path)
            .await
            .expect("remove exited updater socket");
    });
    // Without the marker, the ordinary startup path reports unsupported. A
    // retry that only waits for a successor would time out instead.
    std::fs::remove_file(home.path().join("packages/standalone/auto-update-version"))
        .expect("remove latest marker");
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::manual_update::request(&daemon),
    )
    .await
    .expect("retry should return to normal startup")
    .expect("unsupported response");
    assert_eq!(result.status, UpdateStatus::Unsupported);
    server.await.expect("updater task");
}

#[cfg(unix)]
#[tokio::test]
async fn unsupported_request_preserves_updater_schedule() {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let home = TempDir::new().expect("home");
    let (daemon, _) = manual_update_daemon(&home);
    let daemon = std::sync::Arc::new(daemon);
    let identity = executable_identity(&daemon.managed_codex_bin)
        .await
        .expect("updater identity");
    let socket_path = daemon.manual_update_socket_path();
    let http = FakeInstallerHttp::new(InstallerResponse::Success(Vec::new()));
    let updater_daemon = std::sync::Arc::clone(&daemon);
    let worker =
        tokio::spawn(async move { super::run_with_http(&http, &updater_daemon, &identity).await });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !socket_path.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "updater did not listen"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    std::fs::remove_file(home.path().join("packages/standalone/auto-update-version"))
        .expect("remove latest selection");
    let mut malformed = codex_uds::UnixStream::connect(&socket_path)
        .await
        .expect("connect malformed request");
    malformed
        .write_all(b"upd")
        .await
        .expect("send partial request");
    malformed.shutdown().await.expect("disconnect request");
    let mut discarded = Vec::new();
    malformed
        .read_to_end(&mut discarded)
        .await
        .expect("rejected request");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!worker.is_finished(), "malformed request stopped updater");
    assert_eq!(
        super::manual_update::request(&daemon)
            .await
            .expect("unsupported response")
            .status,
        UpdateStatus::Unsupported
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!worker.is_finished(), "unsupported request stopped updater");
    worker.abort();
}

#[cfg(unix)]
async fn test_control_server(
    daemon: &Daemon,
    home: &std::path::Path,
) -> tokio::task::JoinHandle<()> {
    use futures::SinkExt;
    use futures::StreamExt;
    std::fs::create_dir_all(daemon.socket_path.parent().expect("socket parent"))
        .expect("socket directory");
    let mut listener = codex_uds::UnixListener::bind(&daemon.socket_path)
        .await
        .expect("control listener");
    let codex_home = home.to_path_buf();
    tokio::spawn(async move {
        loop {
            let connection = listener.accept().await.expect("control connection");
            let mut websocket = tokio_tungstenite::accept_async(connection)
                .await
                .expect("websocket handshake");
            websocket
                .next()
                .await
                .expect("initialize request")
                .expect("frame");
            let version = if std::fs::read_to_string(
                crate::managed_install::package_root(&codex_home).join("auto-update-version"),
            )
            .expect("selected release")
            .starts_with("1.1.0")
            {
                "1.1.0"
            } else {
                "1.0.0"
            };
            websocket.send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::json!({"id": 1, "result": {
                    "userAgent": format!("codex_app_server_daemon/{version}"),
                    "codexHome": codex_home, "platformFamily": "unix", "platformOs": std::env::consts::OS,
                }}).to_string().into(),
            )).await.expect("initialize response");
            websocket
                .next()
                .await
                .expect("initialized notification")
                .expect("frame");
        }
    })
}

#[cfg(unix)]
#[tokio::test]
async fn manual_update_restarts_managed_daemon_with_automatic_updates_disabled() {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let home = TempDir::new().expect("home");
    let (daemon, release) = manual_update_daemon(&home);
    let daemon = std::sync::Arc::new(daemon);
    std::fs::create_dir_all(daemon.settings_file.parent().expect("state directory"))
        .expect("state directory");
    std::fs::write(
        &daemon.settings_file,
        r#"{"updater":{"autoUpdateEnabled":false}}"#,
    )
    .expect("disabled updater");
    let server = test_control_server(&daemon, home.path()).await;
    let settings = crate::settings::DaemonSettings::default();
    let backend = crate::backend::pid_backend(daemon.backend_paths(&settings));
    backend.start().await.expect("start daemon");
    let current_pid = || {
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(&daemon.pid_file).expect("daemon PID record"),
        )
        .expect("PID JSON")["pid"]
            .as_u64()
            .expect("PID")
    };
    let before = current_pid();
    let next = release.replacen("1.0.0", "1.1.0", 1);
    let standalone = home.path().join("packages/standalone");
    let ready = home.path().join("installer-ready");
    let proceed = home.path().join("installer-proceed");
    let script = format!(
        "#!/bin/sh\n# CODEX_INSTALL_IF_LATEST\ntest \"$CODEX_INSTALL_IF_LATEST\" = 1 || exit 4\nif [ \"$CODEX_UPDATE_FROM_RELEASE\" = '{next}' ]; then exit 0; fi\ntest \"$CODEX_UPDATE_FROM_RELEASE\" = '{release}' || exit 5\ntouch '{ready}'\nwhile [ ! -e '{proceed}' ]; do sleep .05; done\nmkdir -p '{root}/releases/{next}/bin'\nprintf '#!/bin/sh\\nif [ \"$1\" = --version ]; then echo codex 1.1.0; else exec sleep 30; fi\\n' > '{root}/releases/{next}/bin/codex'\nchmod +x '{root}/releases/{next}/bin/codex'\nln -sfn 'releases/{next}' '{root}/current'\nprintf '{next}' > '{root}/auto-update-version'\n",
        root = standalone.display(),
        ready = ready.display(),
        proceed = proceed.display(),
    );
    let http = FakeInstallerHttp::new(InstallerResponse::Success(script.into_bytes()));
    let updater_daemon = std::sync::Arc::clone(&daemon);
    let worker = tokio::spawn(async move {
        super::run_with_http(
            &http,
            &updater_daemon,
            &executable_identity_from_bytes(b"updater"),
        )
        .await
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !daemon.manual_update_socket_path().exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "updater did not listen"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let request_daemon = std::sync::Arc::clone(&daemon);
    let request = tokio::spawn(async move { super::manual_update::request(&request_daemon).await });
    while !ready.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "installer did not start"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let mut queued = codex_uds::UnixStream::connect(&daemon.manual_update_socket_path())
        .await
        .expect("queue a second update");
    queued
        .write_all(b"update\n")
        .await
        .expect("send second update");
    std::fs::write(proceed, b"go").expect("release installer");
    let output = request
        .await
        .expect("first request task")
        .expect("manual update");
    assert_eq!(output.status, UpdateStatus::Updated);
    assert_eq!(output.installed_version.as_deref(), Some("1.1.0"));
    assert_eq!(output.running_version.as_deref(), Some("1.1.0"));
    assert_eq!(
        output.managed_codex_path,
        standalone.join("current/bin/codex")
    );
    let restarted = current_pid();
    assert_ne!(restarted, before);
    let mut response = Vec::new();
    queued
        .read_to_end(&mut response)
        .await
        .expect("second response");
    let second: Result<crate::UpdateOutput, String> =
        serde_json::from_slice(&response).expect("valid second response");
    assert_eq!(
        second.expect("second update").status,
        UpdateStatus::NoUpdate
    );
    assert_eq!(current_pid(), restarted);
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .expect("one-shot updater did not exit")
        .expect("updater task")
        .expect("updater loop");
    let no_op = FakeInstallerHttp::new(InstallerResponse::Success(
        b"#!/bin/sh\n# CODEX_INSTALL_IF_LATEST\nexit 0\n".to_vec(),
    ));
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(
            daemon
                .current_managed_codex_bin()
                .expect("current executable"),
        )
        .expect("managed executable")
        .write_all(b"\n# same-version replacement\n")
        .expect("replace binary bytes");
    let output = manual_update_once(
        &no_op,
        &daemon,
        &executable_identity_from_bytes(b"updater"),
        &mut test_terminate(),
    )
    .await
    .expect("retry with stale running binary");
    assert_eq!(output.status, UpdateStatus::NoUpdate);
    assert_ne!(current_pid(), restarted);
    backend.stop().await.expect("stop daemon");
    server.abort();
}

#[cfg(windows)]
#[tokio::test]
async fn powershell_installer_is_noninteractive_and_reports_script_failure() {
    let valid = FakeInstallerHttp::new(InstallerResponse::Success(
        br#"
function Test-Installer {
    if ($env:CODEX_NON_INTERACTIVE -ne '1') { throw 'interactive installer' }
    if ($env:CODEX_INSTALL_DAEMON_ONLY -ne '1') { throw 'wrong package destination' }
}
Test-Installer
"#
        .to_vec(),
    ));
    let script = super::fetch_installer_script(&valid)
        .await
        .expect("fetch installer");
    super::run_installer_script(
        &script,
        super::InstallerMode::Update("0.150.0-x86_64-pc-windows-msvc"),
        std::path::Path::new("packages/app-server-daemon"),
    )
    .await
    .expect("installer succeeds");
    let failing = FakeInstallerHttp::new(InstallerResponse::Success(
        b"throw 'installer failed'".to_vec(),
    ));
    let script = super::fetch_installer_script(&failing)
        .await
        .expect("fetch failing installer");
    assert!(
        super::run_installer_script(
            &script,
            super::InstallerMode::Update("0.150.0-x86_64-pc-windows-msvc"),
            std::path::Path::new("packages/app-server-daemon")
        )
        .await
        .is_err()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn update_rejects_a_package_root_change_during_download() {
    struct ChangingRoot<'a>(&'a std::path::Path);
    impl InstallerHttp for ChangingRoot<'_> {
        async fn get(&self, _url: &str) -> anyhow::Result<InstallerResponse> {
            std::os::unix::fs::symlink(
                self.0.join("packages/standalone"),
                self.0.join("packages/app-server-daemon"),
            )?;
            Ok(InstallerResponse::Success(
                b"# CODEX_INSTALL_IF_LATEST\nexit 99\n".to_vec(),
            ))
        }
    }
    let home = TempDir::new().unwrap();
    let (daemon, _) = manual_update_daemon(&home);
    let identity = executable_identity(&daemon.managed_codex_bin)
        .await
        .unwrap();
    let error = manual_update_once(
        &ChangingRoot(home.path()),
        &daemon,
        &identity,
        &mut test_terminate(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("package root changed"));
}

#[cfg(unix)]
#[tokio::test]
async fn daemon_owned_updates_require_and_request_an_isolated_installer() {
    let home = TempDir::new().unwrap();
    let (mut daemon, _) = manual_update_daemon(&home);
    let root = home.path().join("packages/app-server-daemon");
    std::fs::rename(home.path().join("packages/standalone"), &root).unwrap();
    daemon.managed_codex_bin = root.join("current/codex");
    let identity = executable_identity(&daemon.managed_codex_bin)
        .await
        .unwrap();
    let old = FakeInstallerHttp::new(InstallerResponse::Success(
        b"# CODEX_INSTALL_IF_LATEST\nexit 0\n".to_vec(),
    ));
    let error = manual_update_once(&old, &daemon, &identity, &mut test_terminate())
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("does not support daemon-owned packages")
    );
    let current = FakeInstallerHttp::new(InstallerResponse::Success(
        b"# CODEX_INSTALL_IF_LATEST\ntest \"$CODEX_INSTALL_DAEMON_ONLY\" = 1\n".to_vec(),
    ));
    let output = manual_update_once(&current, &daemon, &identity, &mut test_terminate())
        .await
        .unwrap();
    assert_eq!(output.status, UpdateStatus::NoUpdate);
    assert!(!home.path().join("packages/standalone").exists());
}
