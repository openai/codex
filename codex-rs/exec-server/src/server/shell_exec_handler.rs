use std::time::Duration;

use codex_app_server_protocol::JSONRPCErrorError;
use uuid::Uuid;

use crate::protocol::ExecOutputStream;
use crate::protocol::ExecParams;
use crate::protocol::ReadParams;
use crate::protocol::ShellExecParams;
use crate::protocol::ShellExecResponse;
use crate::protocol::TerminateParams;
use crate::rpc::invalid_params;
use crate::server::process_handler::ProcessHandler;

const DEFAULT_EXEC_TIMEOUT_MS: u64 = 10_000;
const EXEC_TIMEOUT_EXIT_CODE: i32 = 124;
const READ_CHUNK_SIZE: usize = 8192;

#[derive(Clone)]
pub(crate) struct ShellExecHandler {
    process: ProcessHandler,
}

impl ShellExecHandler {
    pub(crate) fn new(process: ProcessHandler) -> Self {
        Self { process }
    }

    pub(crate) async fn exec(
        &self,
        params: ShellExecParams,
    ) -> Result<ShellExecResponse, JSONRPCErrorError> {
        self.process.require_initialized_for("shell execution")?;

        if params.command.is_empty() {
            return Err(invalid_params("command must not be empty".to_string()));
        }

        let process_id = format!("shell-exec-{}", Uuid::new_v4());
        self.process
            .exec(ExecParams {
                process_id: process_id.clone(),
                argv: params.command,
                cwd: params.cwd,
                env: params.env,
                tty: false,
                arg0: params.arg0,
            })
            .await?;

        let retained_bytes_cap = params.output_bytes_cap;
        let timeout = Duration::from_millis(params.timeout_ms.unwrap_or(DEFAULT_EXEC_TIMEOUT_MS));
        let expiration_wait = tokio::time::sleep(timeout);
        tokio::pin!(expiration_wait);

        let mut stdout = Vec::with_capacity(
            retained_bytes_cap.map_or(READ_CHUNK_SIZE, |max_bytes| READ_CHUNK_SIZE.min(max_bytes)),
        );
        let mut stderr = Vec::with_capacity(stdout.capacity());
        let mut after_seq = None;
        let mut exit_code = None;
        let mut timed_out = false;

        loop {
            let read_future = self.process.exec_read(ReadParams {
                process_id: process_id.clone(),
                after_seq,
                max_bytes: Some(READ_CHUNK_SIZE),
                wait_ms: Some(50),
            });
            tokio::pin!(read_future);

            let read_response = tokio::select! {
                response = &mut read_future => response?,
                _ = &mut expiration_wait => {
                    timed_out = true;
                    let _ = self.process.terminate(TerminateParams {
                        process_id: process_id.clone(),
                    }).await;
                    break;
                }
            };

            after_seq = Some(read_response.next_seq.saturating_sub(1));
            append_process_output(
                read_response.chunks,
                &mut stdout,
                &mut stderr,
                retained_bytes_cap,
            );

            if read_response.exited {
                exit_code = Some(read_response.exit_code.unwrap_or(-1));
                loop {
                    let drain_response = self
                        .process
                        .exec_read(ReadParams {
                            process_id: process_id.clone(),
                            after_seq,
                            max_bytes: Some(READ_CHUNK_SIZE),
                            wait_ms: Some(0),
                        })
                        .await?;
                    if drain_response.chunks.is_empty() {
                        break;
                    }
                    after_seq = Some(drain_response.next_seq.saturating_sub(1));
                    append_process_output(
                        drain_response.chunks,
                        &mut stdout,
                        &mut stderr,
                        retained_bytes_cap,
                    );
                }
                break;
            }
        }

        let aggregated_output = aggregate_output(&stdout, &stderr, retained_bytes_cap);
        Ok(ShellExecResponse {
            exit_code: exit_code.unwrap_or(EXEC_TIMEOUT_EXIT_CODE),
            stdout: stdout.into(),
            stderr: stderr.into(),
            aggregated_output: aggregated_output.into(),
            timed_out,
        })
    }
}

fn append_process_output(
    chunks: Vec<crate::protocol::ProcessOutputChunk>,
    stdout: &mut Vec<u8>,
    stderr: &mut Vec<u8>,
    retained_bytes_cap: Option<usize>,
) {
    for chunk in chunks {
        let bytes = chunk.chunk.into_inner();
        match chunk.stream {
            ExecOutputStream::Stderr => append_with_cap(stderr, &bytes, retained_bytes_cap),
            ExecOutputStream::Stdout | ExecOutputStream::Pty => {
                append_with_cap(stdout, &bytes, retained_bytes_cap)
            }
        }
    }
}

fn append_with_cap(dst: &mut Vec<u8>, src: &[u8], max_bytes: Option<usize>) {
    if let Some(max_bytes) = max_bytes {
        append_capped(dst, src, max_bytes);
    } else {
        dst.extend_from_slice(src);
    }
}

fn append_capped(dst: &mut Vec<u8>, src: &[u8], max_bytes: usize) {
    if dst.len() >= max_bytes {
        return;
    }
    let remaining = max_bytes.saturating_sub(dst.len());
    let take = remaining.min(src.len());
    dst.extend_from_slice(&src[..take]);
}

fn aggregate_output(stdout: &[u8], stderr: &[u8], max_bytes: Option<usize>) -> Vec<u8> {
    let Some(max_bytes) = max_bytes else {
        let total_len = stdout.len().saturating_add(stderr.len());
        let mut aggregated = Vec::with_capacity(total_len);
        aggregated.extend_from_slice(stdout);
        aggregated.extend_from_slice(stderr);
        return aggregated;
    };

    let total_len = stdout.len().saturating_add(stderr.len());
    let mut aggregated = Vec::with_capacity(total_len.min(max_bytes));

    if total_len <= max_bytes {
        aggregated.extend_from_slice(stdout);
        aggregated.extend_from_slice(stderr);
        return aggregated;
    }

    let want_stdout = stdout.len().min(max_bytes / 3);
    let want_stderr = stderr.len();
    let stderr_take = want_stderr.min(max_bytes.saturating_sub(want_stdout));
    let remaining = max_bytes.saturating_sub(want_stdout + stderr_take);
    let stdout_take = want_stdout + remaining.min(stdout.len().saturating_sub(want_stdout));

    aggregated.extend_from_slice(&stdout[..stdout_take]);
    aggregated.extend_from_slice(&stderr[..stderr_take]);
    aggregated
}
