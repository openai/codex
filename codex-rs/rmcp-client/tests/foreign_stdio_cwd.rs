use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use codex_exec_server::ExecBackend;
use codex_exec_server::ExecBackendFuture;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecServerError;
use codex_rmcp_client::ExecutorStdioServerLauncher;
use codex_rmcp_client::RmcpClient;
use codex_utils_path_uri::LegacyAppPathString;
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
    let cwd = LegacyAppPathString::from_path(Path::new(cwd));
    let expected_cwd: PathUri = cwd
        .clone()
        .try_into()
        .expect("foreign absolute cwd should convert to a path URI");
    let backend = Arc::new(RecordingExecBackend::default());
    let launcher = Arc::new(ExecutorStdioServerLauncher::new(backend.clone()));

    let error = match RmcpClient::new_stdio_client(
        OsString::from("echo"),
        Vec::new(),
        /*env*/ None,
        &[],
        Some(cwd),
        launcher,
    )
    .await
    {
        Ok(_) => panic!("recording backend should stop executor launch"),
        Err(error) => error,
    };

    assert!(
        error
            .to_string()
            .contains("stop after recording executor request")
    );
    let params = backend
        .params
        .lock()
        .expect("recorded params lock should not be poisoned")
        .take()
        .expect("executor start request should be recorded");
    assert_eq!(params.cwd, expected_cwd);
}
