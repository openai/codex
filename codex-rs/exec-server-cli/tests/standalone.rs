//! Exercise the shipped executor through its public stdio interface, including child re-exec.

use anyhow::Context;
use codex_exec_server::FileSystemSandboxContext;
#[cfg(unix)]
use codex_protocol::models::PermissionProfile;
#[cfg(unix)]
use codex_protocol::permissions::FileSystemAccessMode;
#[cfg(unix)]
use codex_protocol::permissions::FileSystemSandboxEntry;
#[cfg(unix)]
use codex_protocol::permissions::NetworkSandboxPolicy;
#[cfg(unix)]
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cargo_bin::cargo_bin;
use codex_utils_process::background_command;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn registration_bearer_rejects_plaintext_remote_registry() -> anyhow::Result<()> {
    let binary = cargo_bin("codex-exec-server")?;
    let home = tempfile::tempdir()?;
    let mut child = tokio::process::Command::from(background_command(&binary))
        .args([
            "--remote",
            "http://registry.example.invalid",
            "--environment-id",
            "test",
            "--registration-bearer-stdin",
        ])
        .env("CODEX_HOME", home.path())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    child
        .stdin
        .take()
        .context("registration stdin")?
        .write_all(b"registration-placeholder\n")
        .await?;
    let output = child.wait_with_output().await?;
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)?.contains("requires https for non-loopback endpoints")
    );
    Ok(())
}

#[tokio::test]
async fn registration_uses_its_bearer_before_and_after_desktop_sign_in() -> anyhow::Result<()> {
    let binary = cargo_bin("codex-exec-server")?;
    let home = tempfile::tempdir()?;
    for (desktop_token, account_id) in [
        ("access_token_initial", None),
        ("signed-in-token", Some("desktop-account")),
    ] {
        std::fs::write(
            home.path().join("auth.json"),
            serde_json::to_vec(&json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "id_token": "eyJhbGciOiJub25lIn0.e30.signature",
                    "access_token": desktop_token,
                    "refresh_token": "refresh_token",
                    "account_id": account_id
                }
            }))?,
        )?;
        let registry = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cloud/environment/test/register"))
            .and(header("Authorization", "Bearer registration-placeholder"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "environment_id": "test",
                "url": format!("{}/relay", registry.uri().replacen("http://", "ws://", 1)),
                "security_profile": "noise_hybrid_ik_v1",
                "executor_registration_id": "registration-1"
            })))
            .mount(&registry)
            .await;
        // Connecting to the returned relay proves the executor accepted registration.
        let relay = Mock::given(method("GET"))
            .and(path("/relay"))
            .respond_with(ResponseTemplate::new(403))
            .expect(1)
            .mount_as_scoped(&registry)
            .await;
        let mut server = tokio::process::Command::from(background_command(&binary))
            .args(["--remote", &registry.uri(), "--environment-id", "test"])
            .arg("--registration-bearer-stdin")
            .stdin(Stdio::piped())
            .env("CODEX_HOME", home.path())
            .kill_on_drop(true)
            .spawn()?;
        server
            .stdin
            .take()
            .context("registration stdin")?
            .write_all(b"registration-placeholder\n")
            .await?;
        tokio::time::timeout(
            std::time::Duration::from_secs(15),
            relay.wait_until_satisfied(),
        )
        .await?;
        #[cfg(target_os = "linux")]
        {
            let pid = server.id().context("executor PID")?;
            let error = std::fs::File::open(format!("/proc/{pid}/environ")).unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        }
    }
    Ok(())
}

#[tokio::test]
async fn standalone_initializes_reports_version_and_runs_helpers() -> anyhow::Result<()> {
    let binary = cargo_bin("codex-exec-server")?;
    let directory = tempfile::tempdir()?;
    let cwd = url::Url::from_directory_path(directory.path())
        .map_err(|()| anyhow::anyhow!("file URI"))?;
    let version = tokio::process::Command::from(background_command(&binary))
        .arg("--version")
        .env("CODEX_HOME", directory.path())
        .output()
        .await?;
    assert!(version.status.success(), "{version:?}");
    assert_eq!(
        String::from_utf8(version.stdout)?,
        format!("codex-exec-server {}\n", env!("CARGO_PKG_VERSION"))
    );

    // The unsandboxed case also runs on Windows. Unix additionally verifies that
    // sandboxed children and filesystem helpers can re-execute the standalone binary.
    #[cfg(not(unix))]
    let sandboxes: Vec<Option<FileSystemSandboxContext>> = vec![None];
    #[cfg(unix)]
    let sandboxes = {
        let mut sandboxes = vec![None];
        if codex_sandboxing::system_bwrap_warning(&PermissionProfile::read_only(), directory.path())
            .is_none()
        {
            // Restrict reads as well, so fs/readFile must dispatch the filesystem helper.
            let hidden = directory.path().join("hidden.txt");
            std::fs::write(&hidden, "private")?;
            let mut policy = PermissionProfile::read_only().file_system_sandbox_policy();
            policy.entries.push(FileSystemSandboxEntry::new(
                AbsolutePathBuf::try_from(hidden)?.into(),
                FileSystemAccessMode::Deny,
            ));
            let permissions = PermissionProfile::from_runtime_permissions(
                &policy,
                NetworkSandboxPolicy::Restricted,
            );
            sandboxes.push(Some(FileSystemSandboxContext::from_permission_profile(
                permissions,
                cwd.as_str().parse()?,
            )));
        } else {
            eprintln!("sandbox case unavailable: system bubblewrap cannot run");
        }
        sandboxes
    };
    for sandbox in sandboxes {
        let mut child = tokio::process::Command::from(background_command(&binary))
            .args(["--listen", "stdio"])
            .current_dir(directory.path())
            .env("CODEX_HOME", directory.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let mut stdin = child.stdin.take().context("executor stdin")?;
        let mut stdout = BufReader::new(child.stdout.take().context("executor stdout")?).lines();
        let request =
            json!({"id": 1, "method": "initialize", "params": {"clientName": "standalone-test"}});
        stdin.write_all(format!("{request}\n").as_bytes()).await?;
        let response: Value =
            serde_json::from_str(&stdout.next_line().await?.context("initialize response")?)?;
        assert_eq!(response["id"], 1);
        assert!(response["result"]["sessionId"].is_string(), "{response}");
        let environment = response["result"]["environmentInfo"].clone();
        assert_eq!(environment["executorVersion"], env!("CARGO_PKG_VERSION"));
        let initialized = json!({"method": "initialized", "params": {}});
        let info = json!({"id": 2, "method": "environment/info", "params": {}});
        stdin
            .write_all(format!("{initialized}\n{info}\n").as_bytes())
            .await?;
        let response: Value =
            serde_json::from_str(&stdout.next_line().await?.context("environment response")?)?;
        assert_eq!(response, json!({"id": 2, "result": environment}));

        let file = directory.path().join("readable.txt");
        std::fs::write(&file, "ok")?;
        let read = json!({"id": 3, "method": "fs/readFile", "params": {
            "path": url::Url::from_file_path(&file).unwrap(), "sandbox": sandbox,
        }});
        stdin.write_all(format!("{read}\n").as_bytes()).await?;
        let response: Value =
            serde_json::from_str(&stdout.next_line().await?.context("read response")?)?;
        assert_eq!(response, json!({"id": 3, "result": {"dataBase64": "b2s="}}));

        #[cfg(windows)]
        let argv = vec!["cmd.exe", "/C", "exit 0"];
        #[cfg(not(windows))]
        let argv = vec![
            "/bin/sh",
            "-c",
            "if touch forbidden 2>/dev/null; then exit 7; fi",
        ];
        #[cfg(not(windows))]
        let expected_exit = if sandbox.is_some() { 0 } else { 7 };
        #[cfg(windows)]
        let expected_exit = 0;
        let start = json!({"id": 4, "method": "process/start", "params": {
            "processId": "standalone-process", "cwd": cwd, "sandbox": sandbox,
            "argv": argv, "env": {"PATH": std::env::var("PATH")?}, "tty": false, "pipeStdin": false, "arg0": null,
        }});
        stdin.write_all(format!("{start}\n").as_bytes()).await?;
        let mut started = false;
        let mut exit_code = None;
        let mut notifications = Vec::new();
        while !started || exit_code.is_none() {
            let response: Value =
                serde_json::from_str(&stdout.next_line().await?.context("process response")?)?;
            if response["id"] == 4 {
                assert!(response.get("error").is_none(), "{response}");
                started = true;
            }
            if response["method"] == "process/exited" {
                exit_code = response["params"]["exitCode"].as_i64();
            }
            notifications.push(response);
        }
        assert_eq!(exit_code, Some(expected_exit), "{notifications:#?}");
        drop(stdin);
        assert!(child.wait().await?.success());
    }
    Ok(())
}
