use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::runtime::ExecuteRequest;
use crate::runtime::RuntimeResponse;
use crate::runtime::WaitRequest;

use super::CodeModeService;
use super::SessionControlCommand;
use super::StartedExecution;

#[derive(Clone, Debug)]
pub struct ExecuteUntilDoneRequest {
    pub execute: ExecuteRequest,
    pub poll_yield_time_ms: u64,
    pub terminate_on_drop: bool,
}

impl CodeModeService {
    pub async fn execute_until_done(
        &self,
        request: ExecuteUntilDoneRequest,
    ) -> Result<RuntimeResponse, String> {
        let StartedExecution {
            control_tx,
            response_rx,
        } = self.start_execution(request.execute).await?;
        let mut drop_guard = RunningCellDropGuard {
            control_tx,
            is_armed: true,
            terminate_on_drop: request.terminate_on_drop,
        };
        let mut accumulated_content_items = Vec::new();
        let mut response = response_rx
            .await
            .map_err(|_| "exec runtime ended unexpectedly".to_string())?;

        loop {
            match response {
                RuntimeResponse::Yielded {
                    cell_id,
                    content_items,
                } => {
                    accumulated_content_items.extend(content_items);
                    response = self
                        .wait(WaitRequest {
                            cell_id,
                            yield_time_ms: request.poll_yield_time_ms,
                            terminate: false,
                        })
                        .await?;
                }
                RuntimeResponse::Terminated {
                    cell_id,
                    content_items,
                } => {
                    accumulated_content_items.extend(content_items);
                    drop_guard.disarm();
                    return Ok(RuntimeResponse::Terminated {
                        cell_id,
                        content_items: accumulated_content_items,
                    });
                }
                RuntimeResponse::Result {
                    cell_id,
                    content_items,
                    stored_values,
                    error_text,
                } => {
                    accumulated_content_items.extend(content_items);
                    drop_guard.disarm();
                    return Ok(RuntimeResponse::Result {
                        cell_id,
                        content_items: accumulated_content_items,
                        stored_values,
                        error_text,
                    });
                }
            }
        }
    }
}

struct RunningCellDropGuard {
    control_tx: mpsc::UnboundedSender<SessionControlCommand>,
    is_armed: bool,
    terminate_on_drop: bool,
}

impl RunningCellDropGuard {
    fn disarm(&mut self) {
        self.is_armed = false;
    }
}

impl Drop for RunningCellDropGuard {
    fn drop(&mut self) {
        if !self.terminate_on_drop || !self.is_armed {
            return;
        }
        let (response_tx, _response_rx) = oneshot::channel();
        let _ = self
            .control_tx
            .send(SessionControlCommand::Terminate { response_tx });
    }
}
