//! A shared supervisor must preserve helper isolation and clean up when its controller exits.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::Lines;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tokio::time::Duration;
use tokio::time::timeout;

type Output = Lines<BufReader<ChildStdout>>;

/// Skip only the macOS environment that cannot apply a nested Seatbelt profile.
#[cfg(target_os = "macos")]
fn sandbox_available() -> Result<bool> {
    let probe = std::process::Command::new("/usr/bin/sandbox-exec")
        .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
        .output()?;
    if !probe.status.success()
        && String::from_utf8_lossy(&probe.stderr)
            .contains("sandbox-exec: sandbox_apply: Operation not permitted")
    {
        eprintln!("skipping helper test: nested Seatbelt is unavailable");
        return Ok(false);
    }
    anyhow::ensure!(probe.status.success(), "Seatbelt probe failed: {probe:?}");
    Ok(true)
}

/// Skip Linux hosts that cannot create the sandbox's required user namespace.
#[cfg(target_os = "linux")]
fn sandbox_available() -> Result<bool> {
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        "default_permissions = \"p\"\n[permissions.p.filesystem]\n\"/\" = \"read\"\n[permissions.p.network]\nenabled = false\n",
    )?;
    let probe = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args(["sandbox", "--", "/usr/bin/true"])
        .env("CODEX_HOME", home.path())
        .output()?;
    let stderr = String::from_utf8_lossy(&probe.stderr);
    if !probe.status.success()
        && (stderr.contains("Operation not permitted")
            || stderr.contains("bwrap: Failed to make / slave: Permission denied"))
    {
        eprintln!("skipping helper test: Linux sandbox is unavailable: {stderr}");
        return Ok(false);
    }
    anyhow::ensure!(
        probe.status.success(),
        "Linux sandbox probe failed: {probe:?}"
    );
    Ok(true)
}

/// Spawn a launcher with a minimal read-only, network-disabled sandbox profile.
fn spawn_launcher(home: &TempDir, stdin: Stdio, stdout: Stdio) -> Result<Child> {
    std::fs::write(
        home.path().join("config.toml"),
        "default_permissions = \"p\"\n[permissions.p.filesystem]\n\"/\" = \"read\"\n[permissions.p.network]\nenabled = false\n",
    )?;
    Ok(Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .arg("--codex-run-as-sandbox-launcher")
        .env("CODEX_HOME", home.path())
        .stdin(stdin)
        .stdout(stdout)
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?)
}

/// Start a launcher with standard pipe-backed controller streams.
async fn start_launcher(home: &TempDir) -> Result<(Child, ChildStdin, Output)> {
    let mut child = spawn_launcher(home, Stdio::piped(), Stdio::piped())?;
    let input = child.stdin.take().context("launcher stdin")?;
    let mut output = BufReader::new(child.stdout.take().context("launcher stdout")?).lines();
    assert_eq!(next_event(&mut output).await?["method"], "ready");
    Ok((child, input, output))
}

/// Read one event, failing on a launcher error or a stalled connection.
async fn next_event<R: AsyncRead + Unpin>(output: &mut Lines<BufReader<R>>) -> Result<Value> {
    let line = timeout(Duration::from_secs(45), output.next_line())
        .await??
        .context("launcher disconnected")?;
    let event: Value = serde_json::from_str(&line)?;
    assert_ne!(event["method"], "error", "{event}");
    Ok(event)
}

#[tokio::test]
async fn launcher_accepts_socket_backed_controller_stdio() -> Result<()> {
    let home = TempDir::new()?;
    let (controller_input, launcher_input) = std::os::unix::net::UnixStream::pair()?;
    let (controller_output, launcher_output) = std::os::unix::net::UnixStream::pair()?;
    controller_input.set_nonblocking(true)?;
    controller_output.set_nonblocking(true)?;
    let mut child = spawn_launcher(
        &home,
        Stdio::from(std::os::fd::OwnedFd::from(launcher_input)),
        Stdio::from(std::os::fd::OwnedFd::from(launcher_output)),
    )?;
    let mut input = tokio::net::UnixStream::from_std(controller_input)?;
    let mut output = BufReader::new(tokio::net::UnixStream::from_std(controller_output)?).lines();
    assert_eq!(next_event(&mut output).await?["method"], "ready");
    let start =
        json!({"method":"start","params":{"id":"","cwd":home.path(),"command":["/usr/bin/true"]}});
    input.write_all(format!("{start}\n").as_bytes()).await?;
    let line = timeout(Duration::from_secs(10), output.next_line())
        .await??
        .context("launcher error event")?;
    let event: Value = serde_json::from_str(&line)?;
    assert_eq!(event["method"], "error");
    assert_eq!(
        event["params"]["message"],
        "invalid or duplicate process id"
    );
    stop_launcher(&mut child, input).await
}

/// Close the controller and require the launcher to exit promptly.
async fn stop_launcher<T>(child: &mut Child, input: T) -> Result<()> {
    drop(input);
    assert!(
        timeout(Duration::from_secs(10), child.wait())
            .await??
            .success()
    );
    Ok(())
}

#[tokio::test]
async fn helper_cwd_symlinks_do_not_skip_project_config() -> Result<()> {
    let home = TempDir::new()?;
    let project = std::fs::canonicalize(home.path())?.join("project");
    std::fs::create_dir_all(project.join("nested"))?;
    std::fs::create_dir(project.join(".codex"))?;
    anyhow::ensure!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&project)
            .status()?
            .success(),
        "git init failed"
    );
    std::fs::write(project.join(".codex/config.toml"), "invalid = [toml")?;
    let alias = home.path().join("alias");
    std::os::unix::fs::symlink(project.join("nested"), &alias)?;
    let (mut child, mut input, mut output) = start_launcher(&home).await?;
    let config = home.path().join("config.toml");
    let mut content = std::fs::read_to_string(&config)?;
    content.push_str(&format!(
        "\n[projects.{}]\ntrust_level = \"trusted\"\n",
        serde_json::to_string(&project)?
    ));
    std::fs::write(config, content)?;
    let start =
        json!({"method":"start","params":{"id":"config","cwd":alias,"command":["/usr/bin/true"]}});
    input.write_all(format!("{start}\n").as_bytes()).await?;
    let line = timeout(Duration::from_secs(45), output.next_line())
        .await??
        .context("launcher error")?;
    let event: Value = serde_json::from_str(&line)?;
    assert_eq!(event["method"], "error");
    assert!(
        event["params"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("config.toml")),
        "{event}"
    );
    stop_launcher(&mut child, input).await
}

#[tokio::test]
async fn helpers_cannot_write_to_each_others_roots() -> Result<()> {
    if !sandbox_available()? {
        return Ok(());
    }
    let home = TempDir::new()?;
    let (mut child, mut input, mut output) = start_launcher(&home).await?;
    let a = home.path().join("a");
    let b = home.path().join("b");
    std::fs::create_dir(&a)?;
    std::fs::create_dir(&b)?;
    for (id, own, other) in [("a", &a, &b), ("b", &b, &a)] {
        let override_arg = format!(
            "permissions.p={{filesystem={{\"/\"=\"read\",\"{}\"=\"write\"}},network={{enabled=false}}}}",
            own.display()
        );
        let script = format!(
            "touch '{}' && ! touch '{}'",
            own.join("allowed").display(),
            other.join(format!("forbidden-{id}")).display()
        );
        let message = json!({"method":"start","params":{"id":id,"cwd":own,"command":["/bin/sh","-c",script],"configOverrides":[override_arg]}});
        input.write_all(format!("{message}\n").as_bytes()).await?;
    }
    input.flush().await?;
    let mut exited = 0;
    while exited < 2 {
        let event = next_event(&mut output).await?;
        if event["method"] == "exited" {
            assert_eq!(
                event["params"]["status"],
                json!({"type":"exit","code":0}),
                "{event}"
            );
            exited += 1;
        }
    }
    assert!(a.join("allowed").exists() && b.join("allowed").exists());
    assert!(!a.join("forbidden-b").exists() && !b.join("forbidden-a").exists());
    stop_launcher(&mut child, input).await
}

#[tokio::test]
async fn stdin_backpressure_preserves_input() -> Result<()> {
    if !sandbox_available()? {
        return Ok(());
    }
    let home = TempDir::new()?;
    let (mut child, mut input, mut output) = start_launcher(&home).await?;
    let start = json!({"method":"start","params":{"id":"input","cwd":home.path(),"command":["/bin/sh","-c","sleep 0.2; wc -c"]}});
    input.write_all(format!("{start}\n").as_bytes()).await?;
    assert_eq!(next_event(&mut output).await?["method"], "started");
    let data = BASE64.encode(vec![b'x'; 16 * 1024]);
    for _ in 0..100 {
        let write = json!({"method":"write","params":{"id":"input","data":data}});
        input.write_all(format!("{write}\n").as_bytes()).await?;
        assert_eq!(next_event(&mut output).await?["method"], "stdinConsumed");
    }
    input
        .write_all(b"{\"method\":\"closeStdin\",\"params\":{\"id\":\"input\"}}\n")
        .await?;
    let event = next_event(&mut output).await?;
    assert_eq!(event["method"], "output");
    let bytes = BASE64.decode(event["params"]["data"].as_str().context("output data")?)?;
    assert_eq!(String::from_utf8(bytes)?.trim(), "1638400");
    assert_eq!(next_event(&mut output).await?["method"], "exited");
    // Bubblewrap keeps the stdin pipe open even after the sandboxed command closes it.
    #[cfg(target_os = "macos")]
    {
        let start = json!({"method":"start","params":{"id":"closed","cwd":home.path(),"command":["/bin/sh","-c","exec 0<&-; echo ready; exec sleep 60"]}});
        input.write_all(format!("{start}\n").as_bytes()).await?;
        assert_eq!(next_event(&mut output).await?["method"], "started");
        assert_eq!(next_event(&mut output).await?["method"], "output");
        let write = json!({"method":"write","params":{"id":"closed","data":BASE64.encode(b"x")}});
        input.write_all(format!("{write}\n").as_bytes()).await?;
        assert_eq!(next_event(&mut output).await?["method"], "stdinClosed");
        let terminate = json!({"method":"terminate","params":{"id":"closed"}});
        input.write_all(format!("{terminate}\n").as_bytes()).await?;
        let event = next_event(&mut output).await?;
        assert_eq!(
            event["params"]["status"],
            json!({"type":"signal","signal":9,"coreDumped":false})
        );
    }
    stop_launcher(&mut child, input).await
}

#[tokio::test]
async fn exited_ids_can_be_reused_immediately() -> Result<()> {
    if !sandbox_available()? {
        return Ok(());
    }
    let home = TempDir::new()?;
    let (mut child, mut input, mut output) = start_launcher(&home).await?;
    let start = json!({"method":"start","params":{"id":"reusable","cwd":home.path(),"command":["/bin/sh","-c","exit 23"]}});
    for _ in 0..2 {
        input.write_all(format!("{start}\n").as_bytes()).await?;
        input.flush().await?;
        loop {
            let event = next_event(&mut output).await?;
            if event["method"] == "exited" {
                assert_eq!(event["params"]["status"], json!({"type":"exit","code":23}));
                break;
            }
        }
    }
    stop_launcher(&mut child, input).await
}

#[tokio::test]
async fn exited_helper_preserves_output_without_waiting_for_its_descendant() -> Result<()> {
    if !sandbox_available()? {
        return Ok(());
    }
    let home = TempDir::new()?;
    let (mut child, mut input, mut output) = start_launcher(&home).await?;
    let marker = home.path().join("sent");
    let override_arg = format!(
        "permissions.p={{filesystem={{\"/\"=\"read\",\"{}\"=\"write\"}},network={{enabled=false}}}}",
        home.path().display()
    );
    let script = format!(
        "/usr/bin/yes & IFS= read -r release; printf complete-response >&2; touch '{}'",
        marker.display()
    );
    let start = json!({"method":"start","params":{"id":"output","cwd":home.path(),"command":["/bin/sh","-c",script],"configOverrides":[override_arg]}});
    input.write_all(format!("{start}\n").as_bytes()).await?;
    while next_event(&mut output).await?["method"] != "output" {}
    // Hold controller output while the descendant fills the launcher queue.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let release =
        json!({"method":"write","params":{"id":"output","data":BASE64.encode(b"release\n")}});
    input.write_all(format!("{release}\n").as_bytes()).await?;
    timeout(Duration::from_secs(10), async {
        while !marker.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    // The old supervisor truncated output after its fixed 100 ms drain deadline.
    tokio::time::sleep(Duration::from_millis(250)).await;
    let stderr = timeout(Duration::from_secs(10), async {
        let mut stderr = Vec::new();
        loop {
            let event = next_event(&mut output).await?;
            if event["method"] == "output" {
                let consumed =
                    json!({"method":"outputConsumed","params":{"id":event["params"]["id"]}});
                input.write_all(format!("{consumed}\n").as_bytes()).await?;
            }
            if event["method"] == "output" && event["params"]["stream"] == "stderr" {
                stderr.extend(
                    BASE64.decode(event["params"]["data"].as_str().context("stderr chunk")?)?,
                );
            }
            if event["method"] == "exited" {
                assert_eq!(event["params"]["status"], json!({"type":"exit","code":0}));
                break Ok::<_, anyhow::Error>(stderr);
            }
        }
    })
    .await??;
    assert_eq!(stderr, b"complete-response");
    stop_launcher(&mut child, input).await
}

#[tokio::test]
async fn invalid_request_exits_while_controller_stdin_is_open() -> Result<()> {
    let home = TempDir::new()?;
    let (mut child, mut input, _output) = start_launcher(&home).await?;
    input.write_all(b"{invalid}\n").await?;
    input.flush().await?;
    assert!(
        !timeout(Duration::from_secs(10), child.wait())
            .await??
            .success()
    );
    drop(input);
    Ok(())
}

#[tokio::test]
async fn controller_eof_stops_a_launcher_with_blocked_output() -> Result<()> {
    if !sandbox_available()? {
        return Ok(());
    }
    let home = TempDir::new()?;
    let (mut child, mut input, mut output) = start_launcher(&home).await?;
    let start = json!({"method":"start","params":{"id":"verbose","cwd":home.path(),"command":["/usr/bin/yes"]}});
    input.write_all(format!("{start}\n").as_bytes()).await?;
    input.flush().await?;
    while next_event(&mut output).await?["method"] != "output" {}
    // Leave stdout open but undrained long enough for the shared output queue to fill.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let writable = home.path().join("second");
    std::fs::create_dir(&writable)?;
    let marker = writable.join("started");
    let override_arg = format!(
        "permissions.p={{filesystem={{\"/\"=\"read\",\"{}\"=\"write\"}},network={{enabled=false}}}}",
        writable.display()
    );
    let script = format!("touch '{}'", marker.display());
    let second = json!({"method":"start","params":{"id":"second","cwd":writable,"command":["/bin/sh","-c",script],"configOverrides":[override_arg]}});
    input.write_all(format!("{second}\n").as_bytes()).await?;
    input.flush().await?;
    timeout(Duration::from_secs(10), async {
        while !marker.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let burst = format!(
        "{}\n",
        json!({"method":"closeStdin","params":{"id":"missing"}})
    )
    .repeat(160);
    input.write_all(burst.as_bytes()).await?;
    // Reusing the live helper's ID gives us a response once the queued requests resume.
    input.write_all(format!("{start}\n").as_bytes()).await?;
    timeout(Duration::from_secs(10), async {
        loop {
            let line = output.next_line().await?.context("launcher event")?;
            let event: Value = serde_json::from_str(&line)?;
            if event["method"] == "output" {
                let consumed =
                    json!({"method":"outputConsumed","params":{"id":event["params"]["id"]}});
                input.write_all(format!("{consumed}\n").as_bytes()).await?;
            }
            if event["method"] == "error" && event["params"]["id"] == "verbose" {
                break Ok::<_, anyhow::Error>(());
            }
        }
    })
    .await??;

    // Saturate the queue again, then disconnect while both dispatch and input are blocked.
    tokio::time::sleep(Duration::from_millis(300)).await;
    input.write_all(format!("{start}\n").as_bytes()).await?;
    input.write_all(burst.as_bytes()).await?;
    stop_launcher(&mut child, input).await?;
    drop(output);
    Ok(())
}
