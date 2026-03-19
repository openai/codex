use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex as StdMutex;

use tokio::process::Child;
use tokio::process::Command;

use crate::client::ExecServerClient;
use crate::client::ExecServerError;
use crate::client_api::ExecServerClientConnectOptions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecServerLaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
}

pub struct SpawnedExecServer {
    client: ExecServerClient,
    child: StdMutex<Option<Child>>,
}

impl SpawnedExecServer {
    pub fn client(&self) -> &ExecServerClient {
        &self.client
    }
}

impl Drop for SpawnedExecServer {
    fn drop(&mut self) {
        if let Ok(mut child_guard) = self.child.lock()
            && let Some(child) = child_guard.as_mut()
        {
            let _ = child.start_kill();
        }
    }
}

pub async fn spawn_local_exec_server(
    command: ExecServerLaunchCommand,
    options: ExecServerClientConnectOptions,
) -> Result<SpawnedExecServer, ExecServerError> {
    let mut child = Command::new(&command.program);
    child.args(&command.args);
    child.args(["--listen", "stdio://"]);
    child.stdin(Stdio::piped());
    child.stdout(Stdio::piped());
    child.stderr(Stdio::inherit());
    child.kill_on_drop(true);

    let mut child = child.spawn().map_err(ExecServerError::Spawn)?;
    let stdin = child.stdin.take().ok_or_else(|| {
        ExecServerError::Protocol("exec-server stdin was not captured".to_string())
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        ExecServerError::Protocol("exec-server stdout was not captured".to_string())
    })?;

    let client = match ExecServerClient::connect_stdio(stdin, stdout, options).await {
        Ok(client) => client,
        Err(err) => {
            let _ = child.start_kill();
            return Err(err);
        }
    };

    Ok(SpawnedExecServer {
        client,
        child: StdMutex::new(Some(child)),
    })
}
