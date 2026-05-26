use std::error::Error;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RuntimeInstallParams;
use codex_app_server_protocol::RuntimeInstallProgressNotification;
use codex_app_server_protocol::RuntimeInstallResponse;
use serde::Deserialize;
use serde::Serialize;
use tokio::io;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio_util::sync::CancellationToken;

use crate::installer::install_runtime_with_progress;

pub const CODEX_RUNTIME_INSTALL_HELPER_ARG1: &str = "--codex-run-as-runtime-install-helper";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuntimeInstallHelperRequest {
    Install { params: RuntimeInstallParams },
    Cancel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuntimeInstallHelperMessage {
    Progress {
        progress: RuntimeInstallProgressNotification,
    },
    Complete {
        response: RuntimeInstallResponse,
    },
    Error {
        error: JSONRPCErrorError,
    },
}

pub fn main() -> ! {
    let exit_code = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => match runtime.block_on(run_main()) {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("runtime install helper failed: {err}");
                1
            }
        },
        Err(err) => {
            eprintln!("failed to start runtime install helper runtime: {err}");
            1
        }
    };
    std::process::exit(exit_code);
}

async fn run_main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut input = BufReader::new(io::stdin()).lines();
    let request = input
        .next_line()
        .await?
        .ok_or("runtime install helper requires an install request")?;
    let RuntimeInstallHelperRequest::Install { params } = serde_json::from_str(&request)? else {
        return Err("runtime install helper first request must be install".into());
    };

    let cancellation = CancellationToken::new();
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
    let install = install_runtime_with_progress(params, progress_tx, cancellation.clone());
    tokio::pin!(install);
    let mut stdout = io::stdout();
    let mut stdin_closed = false;

    loop {
        tokio::select! {
            request = input.next_line(), if !stdin_closed => {
                match request? {
                    Some(request) => match serde_json::from_str(&request)? {
                        RuntimeInstallHelperRequest::Cancel => cancellation.cancel(),
                        RuntimeInstallHelperRequest::Install { .. } => {
                            return Err("runtime install helper accepts one install request".into());
                        }
                    },
                    None => stdin_closed = true
                }
            }
            progress = progress_rx.recv() => {
                if let Some(progress) = progress {
                    write_message(&mut stdout, RuntimeInstallHelperMessage::Progress { progress }).await?;
                }
            }
            response = &mut install => {
                let message = match response {
                    Ok(response) => RuntimeInstallHelperMessage::Complete { response },
                    Err(error) => RuntimeInstallHelperMessage::Error { error },
                };
                write_message(&mut stdout, message).await?;
                return Ok(());
            }
        }
    }
}

async fn write_message(
    stdout: &mut io::Stdout,
    message: RuntimeInstallHelperMessage,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    stdout
        .write_all(serde_json::to_string(&message)?.as_bytes())
        .await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}
