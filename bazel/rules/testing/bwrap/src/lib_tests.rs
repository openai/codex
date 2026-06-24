use std::any::Any;
use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use tempfile::Builder;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::time::Duration;
use tokio::time::timeout;

use super::BwrapTestCommand;
use super::BwrapTestProcess;

fn smoke_fixture() -> Result<PathBuf> {
    Ok(codex_utils_cargo_bin::cargo_bin("bwrap-smoke")?)
}

async fn waiting_smoke_process() -> Result<BwrapTestProcess> {
    let mut process = BwrapTestCommand::new(smoke_fixture()?)
        .arg("wait")
        .spawn()?;
    let mut lines = BufReader::new(process.take_stdout()).lines();
    let ready_line = lines
        .next_line()
        .await?
        .context("bwrap smoke process exited before becoming ready")?;
    assert_eq!(ready_line, "BWRAP_TEST_READY");
    Ok(process)
}

fn environment_path(process: &BwrapTestProcess) -> PathBuf {
    process.environment_path().to_path_buf()
}

fn assert_environment_removed(path: &Path) {
    assert!(
        !path.exists(),
        "bwrap test environment remains: {}",
        path.display()
    );
}

fn assert_panic_message(panic: Box<dyn Any + Send>, expected: &str) {
    assert_eq!(panic.downcast_ref::<&str>(), Some(&expected));
}

async fn assert_future_panics<T>(future: impl Future<Output = T>, expected: &str) {
    let panic = match AssertUnwindSafe(future).catch_unwind().await {
        Ok(_) => panic!("future should panic"),
        Err(panic) => panic,
    };
    assert_panic_message(panic, expected);
}

async fn run_smoke(args: &[&str]) -> Result<BTreeMap<String, String>> {
    let mut command = BwrapTestCommand::new(smoke_fixture()?);
    for arg in args {
        command = command.arg(*arg);
    }
    run_smoke_command(command).await
}

async fn run_smoke_command(command: BwrapTestCommand) -> Result<BTreeMap<String, String>> {
    let mut process = command.spawn()?;
    let mut stdout = process.take_stdout();
    let output = process
        .scope(async move {
            let mut output = String::new();
            stdout.read_to_string(&mut output).await?;
            Ok(output)
        })
        .await?;
    parse_key_values(&output)
}

fn parse_key_values(output: &str) -> Result<BTreeMap<String, String>> {
    output
        .lines()
        .map(|line| {
            line.split_once('=')
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .with_context(|| format!("invalid smoke output line: {line:?}"))
        })
        .collect()
}

fn namespace_identity(namespace: &str) -> Result<String> {
    Ok(fs::read_link(format!("/proc/self/ns/{namespace}"))?
        .to_string_lossy()
        .into_owned())
}

#[tokio::test]
async fn dropping_without_teardown_panics_and_cleans_up() -> Result<()> {
    let process = waiting_smoke_process().await?;
    let environment = environment_path(&process);
    assert_future_panics(
        async move { drop(process) },
        "BwrapTestProcess dropped without async teardown",
    )
    .await;
    assert_environment_removed(&environment);
    Ok(())
}

#[tokio::test]
async fn dropping_while_panicking_does_not_panic_again() -> Result<()> {
    let process = waiting_smoke_process().await?;
    let environment = environment_path(&process);
    assert_future_panics(
        async move {
            let _process = process;
            panic!("sentinel panic");
        },
        "sentinel panic",
    )
    .await;
    assert_environment_removed(&environment);
    Ok(())
}

#[tokio::test]
async fn async_teardown_disarms_drop_bomb_and_cleans_up() -> Result<()> {
    let process = waiting_smoke_process().await?;
    let environment = environment_path(&process);
    process.shutdown().await?;
    assert_environment_removed(&environment);
    Ok(())
}

#[tokio::test]
async fn scope_returns_value_and_cleans_up() -> Result<()> {
    let process = waiting_smoke_process().await?;
    let environment = environment_path(&process);
    let value = process
        .scope(async { Ok::<_, anyhow::Error>("scope value") })
        .await?;
    assert_eq!(value, "scope value");
    assert_environment_removed(&environment);
    Ok(())
}

#[tokio::test]
async fn outer_namespace_topology_matches_the_exec_server_contract() -> Result<()> {
    let values = run_smoke(&["inspect"]).await?;

    assert_eq!(
        values.get("id.effective_uid").map(String::as_str),
        Some("1000")
    );
    assert_eq!(
        values.get("id.effective_gid").map(String::as_str),
        Some("1000")
    );
    for set in ["effective", "permitted", "inheritable", "ambient"] {
        assert_eq!(
            u64::from_str_radix(
                values
                    .get(&format!("cap.{set}"))
                    .with_context(|| format!("missing {set} capabilities"))?,
                16,
            )?,
            0,
            "the non-root outer process must not carry {set} capabilities"
        );
    }
    assert_ne!(
        values.get("ns.user"),
        Some(&namespace_identity("user")?),
        "outer user namespace should differ from its parent"
    );
    for namespace in ["pid", "net"] {
        assert_eq!(
            values.get(&format!("ns.{namespace}")),
            Some(&namespace_identity(namespace)?),
            "outer {namespace} namespace should deliberately match its parent"
        );
    }
    for namespace in ["mnt", "ipc", "uts"] {
        assert_ne!(
            values.get(&format!("ns.{namespace}")),
            Some(&namespace_identity(namespace)?),
            "outer {namespace} namespace should differ from its parent"
        );
    }
    if let Some(pid_one_namespace) = values.get("proc.1.pid_ns") {
        assert_eq!(Some(pid_one_namespace), values.get("proc.self.pid_ns"));
    } else {
        assert!(
            values.contains_key("proc.1.pid_ns.error"),
            "outer procfs should either expose PID 1 or report why it is unreadable"
        );
    }
    assert_eq!(
        values.get("proc.self.pid_ns"),
        Some(&namespace_identity("pid")?),
        "outer /proc should deliberately expose the parent PID namespace"
    );
    for name in ["overflowuid", "overflowgid"] {
        let value = values
            .get(&format!("proc.{name}"))
            .with_context(|| format!("missing {name}"))?;
        assert!(value.parse::<u32>().is_ok(), "invalid {name}: {value:?}");
    }
    Ok(())
}

#[tokio::test]
async fn default_home_and_runtime_directories_use_private_tmp() -> Result<()> {
    let values = run_smoke(&["inspect"]).await?;

    for (name, expected) in [
        ("HOME", "/tmp/home"),
        ("CODEX_HOME", "/tmp/codex-home"),
        ("XDG_RUNTIME_DIR", "/tmp/xdg-runtime"),
        ("TMPDIR", "/tmp"),
    ] {
        assert_eq!(
            values.get(&format!("env.{name}")).map(String::as_str),
            Some(expected)
        );
    }
    Ok(())
}

#[tokio::test]
async fn command_environment_overrides_isolated_defaults() -> Result<()> {
    let overrides = [
        ("HOME", "/tmp/bwrap-test-home-override"),
        ("CODEX_HOME", "/tmp/bwrap-test-codex-home-override"),
        ("XDG_RUNTIME_DIR", "/tmp/bwrap-test-xdg-runtime-override"),
    ];
    let mut command = BwrapTestCommand::new(smoke_fixture()?).arg("inspect");
    for (name, value) in overrides {
        command = command.env(name, value);
    }
    let values = run_smoke_command(command).await?;

    for (name, value) in overrides {
        assert_eq!(
            values.get(&format!("env.{name}")).map(String::as_str),
            Some(value)
        );
    }
    Ok(())
}

#[tokio::test]
async fn root_is_read_only_while_tmp_and_workspace_round_trip() -> Result<()> {
    let tmp_writable = Builder::new()
        .prefix("bwrap-write-smoke-")
        .tempdir_in("/tmp")?;
    fs::write(tmp_writable.path().join("host-sentinel.txt"), "host")?;
    let workspace = std::env::current_dir()?;
    let workspace_writable = Builder::new()
        .prefix("bwrap-workspace-write-smoke-")
        .tempdir_in(workspace)?;
    let root_target = format!("/bwrap-test-root-write-{}", std::process::id());
    let values = run_smoke(&[
        "filesystem",
        tmp_writable
            .path()
            .to_str()
            .context("non-UTF-8 temp path")?,
        workspace_writable
            .path()
            .to_str()
            .context("non-UTF-8 workspace path")?,
        &root_target,
    ])
    .await?;

    assert_eq!(
        values.get("tmp.value").map(String::as_str),
        Some("round-trip")
    );
    assert_eq!(
        values.get("tmp.host_sentinel_visible").map(String::as_str),
        Some("false")
    );
    assert_eq!(
        fs::read_to_string(tmp_writable.path().join("host-sentinel.txt"))?,
        "host"
    );
    assert!(!tmp_writable.path().join("round-trip.txt").exists());
    assert_eq!(
        values.get("workspace.value").map(String::as_str),
        Some("round-trip")
    );
    assert_eq!(
        fs::read_to_string(workspace_writable.path().join("round-trip.txt"))?,
        "round-trip"
    );
    assert_eq!(
        values.get("root.write_errno").map(String::as_str),
        Some(libc::EROFS.to_string().as_str())
    );
    assert!(!Path::new(&root_target).exists());
    Ok(())
}

#[tokio::test]
async fn outer_loopback_reaches_a_parent_listener() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let process = BwrapTestCommand::new(smoke_fixture()?)
        .arg("connect")
        .arg(address.to_string())
        .spawn()?;

    let received = process
        .scope(async move {
            timeout(Duration::from_secs(10), async move {
                let (mut stream, _) = listener.accept().await?;
                let mut received = Vec::new();
                stream.read_to_end(&mut received).await?;
                Ok::<_, anyhow::Error>(received)
            })
            .await
            .context("parent listener timed out")?
        })
        .await?;
    assert_eq!(received, b"BWRAP_LOOPBACK");
    Ok(())
}

#[tokio::test]
async fn raw_nested_bwrap_creates_required_namespaces_and_fresh_proc() -> Result<()> {
    let bwrap = codex_utils_cargo_bin::cargo_bin("bwrap")?;
    let fixture = smoke_fixture()?;
    let values = run_smoke(&[
        "nested",
        bwrap.to_str().context("non-UTF-8 bwrap path")?,
        fixture.to_str().context("non-UTF-8 fixture path")?,
    ])
    .await?;

    for namespace in ["user", "mnt", "pid", "net"] {
        assert_ne!(
            values.get(&format!("outer.ns.{namespace}")),
            values.get(&format!("child.ns.{namespace}")),
            "nested {namespace} namespace should differ from the outer namespace"
        );
    }
    assert_eq!(
        values.get("child.id.effective_uid").map(String::as_str),
        Some("1000")
    );
    assert_eq!(
        values.get("child.id.effective_gid").map(String::as_str),
        Some("1000")
    );
    for set in ["effective", "permitted", "inheritable", "ambient"] {
        assert_eq!(
            u64::from_str_radix(
                values
                    .get(&format!("child.cap.{set}"))
                    .with_context(|| format!("missing nested {set} capabilities"))?,
                16,
            )?,
            0,
            "the nested command should not retain {set} bwrap setup capabilities"
        );
    }
    assert_eq!(
        values
            .get("child.proc.1.pid_ns")
            .context("nested procfs did not expose PID 1")?,
        values
            .get("child.proc.self.pid_ns")
            .context("nested procfs did not expose self")?
    );
    for name in ["overflowuid", "overflowgid"] {
        let value = values
            .get(&format!("child.proc.{name}"))
            .with_context(|| format!("missing nested {name}"))?;
        assert!(value.parse::<u32>().is_ok(), "invalid {name}: {value:?}");
    }
    Ok(())
}
