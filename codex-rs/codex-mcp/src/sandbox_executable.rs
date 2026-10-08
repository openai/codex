//! Verify that the host's re-exec binary supports the sandbox metadata contract.
//! Cache completed probes per absolute executable path; never infer capability from its name or PATH.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;

use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_process::background_command;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;

const PROBE_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 5);
static PROBES: LazyLock<Mutex<HashMap<AbsolutePathBuf, Arc<OnceCell<bool>>>>> =
    LazyLock::new(Mutex::default);

pub(crate) async fn supports_sandbox_state(executable: &AbsolutePathBuf) -> bool {
    let probe = {
        let mut probes = PROBES.lock().await;
        Arc::clone(probes.entry(executable.clone()).or_default())
    };
    *probe
        .get_or_init(|| async {
            matches!(
                tokio::time::timeout(PROBE_TIMEOUT, probe_sandbox_state(executable)).await,
                Ok(Ok(true))
            )
        })
        .await
}

/// Check that the binary accepts the sandbox-state option without launching a sandbox.
///
/// Plain `sandbox --help` can also succeed for unrelated binaries, including Rust test
/// harnesses. Put `--sandbox-state-json` before `--help` so those parsers reject the
/// unknown option. Codex handles help before interpreting the placeholder `{}` policy
/// or loading configuration; only its exit status matters, not the help text.
async fn probe_sandbox_state(executable: &AbsolutePathBuf) -> std::io::Result<bool> {
    let status = tokio::process::Command::from(background_command(executable.as_os_str()))
        .args(["sandbox", "--sandbox-state-json", "{}", "--help"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .status()
        .await?;
    Ok(status.success())
}
