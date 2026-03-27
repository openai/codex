use crate::ipc_framed::decode_bytes;
use crate::ipc_framed::encode_bytes;
use crate::ipc_framed::EmptyPayload;
use crate::ipc_framed::FramedMessage;
use crate::ipc_framed::Message;
use crate::ipc_framed::OutputStream;
use crate::ipc_framed::ResizePayload;
use crate::ipc_framed::StdinPayload;
use anyhow::Result;
use codex_utils_pty::spawn_from_driver;
use codex_utils_pty::ProcessDriver;
use codex_utils_pty::SpawnedProcess;
use codex_utils_pty::TerminalSize;
use std::fs::File;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

pub(crate) fn finish_driver_spawn(driver: ProcessDriver, stdin_open: bool) -> SpawnedProcess {
    let spawned = spawn_from_driver(driver);
    if !stdin_open {
        spawned.session.close_stdin();
    }
    spawned
}

pub(crate) fn normalize_windows_tty_input(bytes: &[u8], previous_was_cr: &mut bool) -> Vec<u8> {
    let mut normalized = Vec::with_capacity(bytes.len());
    for &byte in bytes {
        if byte == b'\n' {
            if !*previous_was_cr {
                normalized.push(b'\r');
            }
            normalized.push(b'\n');
            *previous_was_cr = false;
        } else {
            normalized.push(byte);
            *previous_was_cr = byte == b'\r';
        }
    }
    normalized
}

pub(crate) fn start_runner_pipe_writer(
    mut pipe_write: File,
) -> std::sync::mpsc::Sender<FramedMessage> {
    let (outbound_tx, outbound_rx) = std::sync::mpsc::channel::<FramedMessage>();
    tokio::task::spawn_blocking(move || {
        while let Ok(msg) = outbound_rx.recv() {
            if crate::ipc_framed::write_frame(&mut pipe_write, &msg).is_err() {
                break;
            }
        }
    });
    outbound_tx
}

pub(crate) fn start_runner_stdin_writer(
    mut writer_rx: mpsc::Receiver<Vec<u8>>,
    outbound_tx: std::sync::mpsc::Sender<FramedMessage>,
    normalize_newlines: bool,
    stdin_open: bool,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut previous_was_cr = false;
        while let Some(bytes) = writer_rx.blocking_recv() {
            let bytes = if normalize_newlines {
                normalize_windows_tty_input(&bytes, &mut previous_was_cr)
            } else {
                bytes
            };
            let msg = FramedMessage {
                version: 1,
                message: Message::Stdin {
                    payload: StdinPayload {
                        data_b64: encode_bytes(&bytes),
                    },
                },
            };
            if outbound_tx.send(msg).is_err() {
                break;
            }
        }
        if stdin_open {
            let _ = outbound_tx.send(FramedMessage {
                version: 1,
                message: Message::CloseStdin {
                    payload: EmptyPayload::default(),
                },
            });
        }
    })
}

pub(crate) fn start_runner_stdout_reader(
    mut pipe_read: File,
    stdout_tx: broadcast::Sender<Vec<u8>>,
    stderr_tx: Option<broadcast::Sender<Vec<u8>>>,
    exit_tx: oneshot::Sender<i32>,
) {
    std::thread::spawn(move || loop {
        let msg = match crate::ipc_framed::read_frame(&mut pipe_read) {
            Ok(Some(v)) => v,
            Ok(None) => {
                send_runner_error(
                    "runner pipe closed before exit",
                    &stdout_tx,
                    stderr_tx.as_ref(),
                );
                let _ = exit_tx.send(-1);
                break;
            }
            Err(err) => {
                send_runner_error(
                    &format!("runner read failed: {err}"),
                    &stdout_tx,
                    stderr_tx.as_ref(),
                );
                let _ = exit_tx.send(-1);
                break;
            }
        };

        match msg.message {
            Message::Output { payload } => {
                if let Ok(data) = decode_bytes(&payload.data_b64) {
                    match payload.stream {
                        OutputStream::Stdout => {
                            let _ = stdout_tx.send(data);
                        }
                        OutputStream::Stderr => {
                            if let Some(stderr_tx) = stderr_tx.as_ref() {
                                let _ = stderr_tx.send(data);
                            } else {
                                let _ = stdout_tx.send(data);
                            }
                        }
                    }
                }
            }
            Message::Exit { payload } => {
                let _ = exit_tx.send(payload.exit_code);
                break;
            }
            Message::Error { payload } => {
                send_runner_error(&payload.message, &stdout_tx, stderr_tx.as_ref());
                let _ = exit_tx.send(-1);
                break;
            }
            Message::SpawnReady { .. }
            | Message::Stdin { .. }
            | Message::CloseStdin { .. }
            | Message::Resize { .. }
            | Message::SpawnRequest { .. }
            | Message::Terminate { .. } => {}
        }
    });
}

pub(crate) fn make_runner_resizer(
    outbound_tx: std::sync::mpsc::Sender<FramedMessage>,
) -> Box<dyn FnMut(TerminalSize) -> Result<()> + Send> {
    Box::new(move |size: TerminalSize| {
        outbound_tx
            .send(FramedMessage {
                version: 1,
                message: Message::Resize {
                    payload: ResizePayload {
                        rows: size.rows,
                        cols: size.cols,
                    },
                },
            })
            .map_err(|_| anyhow::anyhow!("runner resize pipe closed"))
    })
}

fn send_runner_error(
    message: &str,
    stdout_tx: &broadcast::Sender<Vec<u8>>,
    stderr_tx: Option<&broadcast::Sender<Vec<u8>>>,
) {
    let formatted = format!("runner error: {message}\n").into_bytes();
    if let Some(stderr_tx) = stderr_tx {
        let _ = stderr_tx.send(formatted);
    } else {
        let _ = stdout_tx.send(formatted);
    }
}
