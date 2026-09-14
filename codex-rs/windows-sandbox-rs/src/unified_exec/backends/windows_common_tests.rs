//! The result-metric hook must preserve terminal exit codes and pipe-failure behavior.

use super::*;
use crate::ipc_framed::ExitPayload;
use crate::ipc_framed::write_frame;
use pretty_assertions::assert_eq;
use std::io::Seek;

#[test]
fn runner_result_reporting_preserves_exit_and_closed_pipe_results() -> Result<()> {
    for exit_code in [Some(0), Some(23), None] {
        let mut frames = tempfile::tempfile()?;
        if let Some(exit_code) = exit_code {
            write_frame(
                &mut frames,
                &FramedMessage {
                    version: IPC_PROTOCOL_VERSION,
                    message: Message::Exit {
                        payload: ExitPayload {
                            exit_code,
                            timed_out: false,
                        },
                    },
                },
            )?;
        }
        frames.rewind()?;
        let (stdout_tx, _stdout_rx) = broadcast::channel(1);
        let (exit_tx, exit_rx) = oneshot::channel();
        start_runner_stdout_reader(frames, stdout_tx, /*stderr_tx*/ None, exit_tx);
        assert_eq!(exit_rx.blocking_recv()?, exit_code.unwrap_or(-1));
    }
    Ok(())
}
