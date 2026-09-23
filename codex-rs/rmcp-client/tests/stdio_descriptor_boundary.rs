//! Unix local MCP servers and their descendants must not inherit unrelated
//! parent descriptors, while their explicit stdio transport remains usable.

#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::LocalStdioServerLauncher;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt as _;
use pretty_assertions::assert_eq;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_stdio_excludes_inheritable_fds_from_server_and_descendant() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let script = temporary.path().join("server.py");
    fs::write(
        &script,
        r#"import errno, json, os, signal, subprocess, sys
signal.alarm(30)
sentinel = int(sys.argv[2])
def probe():
    for fd in (0, 1, 2):
        os.fstat(fd)
    print("descriptor fixture stderr", file=sys.stderr, flush=True)
    try:
        os.fstat(sentinel)
        return True
    except OSError as error:
        if error.errno != errno.EBADF:
            raise
        return False
if sys.argv[1] == "probe":
    print(json.dumps(probe()), flush=True)
    sys.exit(0)
for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")
    if method == "initialize":
        result = {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "descriptor-fixture", "version": "1"}}
    elif method == "tools/call":
        # Python normally closes extra descriptors itself. Disable that cleanup
        # so the descendant actually exercises the MCP launch boundary.
        child = subprocess.run([sys.executable, __file__, "probe", sys.argv[2]], close_fds=False, capture_output=True, text=True, check=True, timeout=5)
        result = {"content": [], "structuredContent": {
            "server": probe(), "descendant": json.loads(child.stdout),
            "descendantStderr": child.stderr}}
    else:
        continue
    print(json.dumps({"jsonrpc": "2.0", "id": message["id"], "result": result}), flush=True)
"#,
    )?;

    let file = tempfile::tempfile()?;
    // F_DUPFD reserves an unused inheritable descriptor without overwriting
    // a descriptor owned by the test runner/runtime.
    let raw = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD, 200) };
    if raw < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: fcntl returned a new descriptor owned only by this test.
    let sentinel = unsafe { OwnedFd::from_raw_fd(raw) };
    let sentinel_arg = raw.to_string();
    let python = which::which("python3")?;
    // Qualify the oracle: the same executable must see the sentinel when
    // spawned without the local MCP descriptor policy.
    let control = Command::new(&python)
        .arg(&script)
        .arg("probe")
        .arg(&sentinel_arg)
        .output()?;
    assert!(control.status.success());
    assert!(serde_json::from_slice::<bool>(&control.stdout)?);

    symlink(&python, temporary.path().join("python-fixture"))?;
    let wrapper = temporary.path().join("executable-text");
    // No shebang: exercise Command's shell fallback after native macOS spawn
    // returns ENOEXEC, including the fallback descriptor cleanup.
    fs::write(&wrapper, "exec \"$@\"\n")?;
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(/*mode*/ 0o755))?;
    for (launch, program, mut args) in [
        ("absolute", python.clone().into_os_string(), Vec::new()),
        ("relative", OsString::from("./python-fixture"), Vec::new()),
        (
            "shell fallback",
            OsString::from("./executable-text"),
            vec![python.clone().into_os_string()],
        ),
    ] {
        args.extend([
            script.clone().into_os_string(),
            "server".into(),
            sentinel_arg.clone().into(),
        ]);
        let client = RmcpClient::new_stdio_client(
            program,
            args,
            /*env*/ None,
            &[],
            Some(temporary.path().to_string_lossy().into_owned()),
            Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?)),
        )
        .await?;
        client
            .initialize(
                InitializeRequestParams::new(
                    ClientCapabilities::default(),
                    Implementation::new("descriptor-boundary-test", "1"),
                )
                .with_protocol_version(ProtocolVersion::V_2025_06_18),
                Some(Duration::from_secs(/*secs*/ 5)),
                Box::new(|_, _| {
                    async {
                        Ok(ElicitationResponse {
                            action: ElicitationAction::Decline,
                            content: None,
                            meta: None,
                        })
                    }
                    .boxed()
                }),
            )
            .await?;
        let result = client
            .call_tool(
                "probe".to_string(),
                /*arguments*/ None,
                /*meta*/ None,
                Some(Duration::from_secs(/*secs*/ 5)),
            )
            .await?;
        client.shutdown().await;
        assert_eq!(
            result.structured_content,
            Some(json!({
                "server": false,
                "descendant": false,
                "descendantStderr": "descriptor fixture stderr\n"
            })),
            "{launch} launch"
        );
    }
    // Cleanup must not close or change the parent's descriptor.
    assert_eq!(
        unsafe { libc::fcntl(sentinel.as_raw_fd(), libc::F_GETFD) },
        0
    );
    Ok(())
}

#[tokio::test]
async fn local_stdio_preserves_exec_failure_reporting() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let script = temporary.path().join("missing-interpreter");
    let interpreter = temporary.path().join("does-not-exist");
    fs::write(&script, format!("#!{}\n", interpreter.display()))?;
    fs::set_permissions(&script, fs::Permissions::from_mode(/*mode*/ 0o755))?;

    // Program resolution succeeds, but exec fails. Descriptor cleanup must
    // preserve the internal error pipe so launch reports the failure directly.
    let result = RmcpClient::new_stdio_client(
        script.into_os_string(),
        Vec::new(),
        /*env*/ None,
        &[],
        /*cwd*/ None,
        Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?)),
    )
    .await;
    assert!(
        result.is_err(),
        "exec failure was reported as a successful launch"
    );
    Ok(())
}
