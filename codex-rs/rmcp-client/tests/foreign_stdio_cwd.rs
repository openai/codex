use std::ffi::OsString;
use std::sync::Arc;
use std::sync::Mutex;

use codex_exec_server::ExecBackend;
use codex_exec_server::ExecBackendFuture;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecServerError;
use codex_rmcp_client::ExecutorStdioServerLauncher;
use codex_rmcp_client::RmcpClient;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

#[derive(Default)]
struct RecordingExecBackend {
    params: Mutex<Option<ExecParams>>,
}

impl ExecBackend for RecordingExecBackend {
    fn start(&self, params: ExecParams) -> ExecBackendFuture<'_> {
        let mut recorded_params = match self.params.lock() {
            Ok(recorded_params) => recorded_params,
            Err(poisoned) => poisoned.into_inner(),
        };
        *recorded_params = Some(params);
        Box::pin(async {
            Err(ExecServerError::Protocol(
                "stop after recording executor request".to_string(),
            ))
        })
    }
}

#[tokio::test]
async fn executor_stdio_forwards_foreign_absolute_cwd_as_path_uri() {
    #[cfg(not(windows))]
    let cwd = r"C:\Users\openai\share";
    #[cfg(windows)]
    let cwd = "/home/openai/share";
    #[cfg(not(windows))]
    let expected_cwd: PathUri = "file:///C:/Users/openai/share"
        .parse()
        .expect("expected cwd should be a path URI");
    #[cfg(windows)]
    let expected_cwd: PathUri = "file:///home/openai/share"
        .parse()
        .expect("expected cwd should be a path URI");
    let backend = Arc::new(RecordingExecBackend::default());
    let launcher = Arc::new(ExecutorStdioServerLauncher::new(backend.clone()));

    let _ = RmcpClient::new_stdio_client(
        OsString::from("echo"),
        Vec::new(),
        /*env*/ None,
        &[],
        Some(cwd.to_string()),
        launcher,
    )
    .await;
    let params = backend
        .params
        .lock()
        .expect("recorded params lock should not be poisoned")
        .take()
        .expect("executor start request should be recorded");
    assert_eq!(params.cwd, expected_cwd);
}
