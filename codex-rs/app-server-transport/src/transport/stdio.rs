//! Stdio transport with EOF cleanup and Unix SIGTERM shutdown. Dedicated I/O
//! threads let SIGTERM abandon blocked pipes without holding the runtime open.
//! A dedicated signal thread arms the shared EOF/SIGTERM deadline even when
//! logging or the Tokio runtime is blocked.

use super::CHANNEL_CAPACITY;
use super::ConnectionOrigin;
use super::TransportEvent;
use super::forward_incoming_message;
use super::next_connection_id;
use super::serialize_outgoing_message;
use crate::outgoing_message::QueuedOutgoingMessage;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCRequest;
use std::io::BufRead;
use std::io::ErrorKind;
use std::io::Result as IoResult;
use std::io::Write;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
use tracing::info;

pub async fn start_stdio_connection(
    transport_event_tx: mpsc::Sender<TransportEvent>,
    initialize_client_name_tx: oneshot::Sender<String>,
    install_shutdown_signal_handler: bool,
) -> IoResult<JoinHandle<()>> {
    let shutdown_signal = CancellationToken::new();
    #[cfg(unix)]
    if install_shutdown_signal_handler {
        use signal_hook::consts::SIGTERM;
        use signal_hook::iterator::Signals;

        // Register before accepting requests. Receiving SIGTERM and arming the
        // watchdog must not depend on Tokio or synchronous transport logging.
        let mut signals = Signals::new([SIGTERM])?;
        let shutdown_signal = shutdown_signal.clone();
        std::thread::Builder::new()
            .name("app-server-signal".to_string())
            .spawn(move || {
                if signals.forever().next().is_some() {
                    start_shutdown_watchdog();
                    shutdown_signal.cancel();
                }
            })?;
    }
    let connection_id = next_connection_id();
    let (writer_tx, mut writer_rx) = mpsc::channel::<QueuedOutgoingMessage>(CHANNEL_CAPACITY);
    let writer_tx_for_reader = writer_tx.clone();
    transport_event_tx
        .send(TransportEvent::ConnectionOpened {
            connection_id,
            origin: ConnectionOrigin::Stdio,
            auth: None,
            writer: writer_tx,
            disconnect_sender: None,
        })
        .await
        .map_err(|_| std::io::Error::new(ErrorKind::BrokenPipe, "processor unavailable"))?;

    // Tokio's stdin uses an uncancellable blocking task, which keeps its runtime
    // alive while the client leaves stdin open. This process-owned thread may
    // remain blocked until process exit, but is not joined by the runtime.
    let (stdin_tx, mut stdin_rx) = mpsc::channel(/*buffer*/ 1);
    std::thread::Builder::new()
        .name("app-server-stdin".to_string())
        .spawn(move || {
            for line in std::io::stdin().lock().lines() {
                if stdin_tx.blocking_send(line).is_err() {
                    break;
                }
            }
        })?;

    // Keep stdout's blocking writes off Tokio's pool too. The forwarding future
    // owns writer_rx so cancelling it also releases producers stuck on a full queue.
    let (stdout_tx, mut stdout_rx) =
        mpsc::channel::<(String, oneshot::Sender<()>)>(/*buffer*/ 1);
    std::thread::Builder::new()
        .name("app-server-stdout".to_string())
        .spawn(move || {
            let mut stdout = std::io::stdout().lock();
            while let Some((json, written_tx)) = stdout_rx.blocking_recv() {
                if let Err(err) = stdout.write_all(json.as_bytes()) {
                    error!("Failed to write to stdout: {err}");
                    break;
                }
                let _ = written_tx.send(());
            }
        })?;

    let transport_event_tx_for_reader = transport_event_tx.clone();
    let read_messages = async move {
        let mut initialize_client_name_tx = Some(initialize_client_name_tx);
        while let Some(line) = stdin_rx.recv().await {
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    error!("Failed reading stdin: {err}");
                    break;
                }
            };
            if let Some(client_name) = stdio_initialize_client_name(&line)
                && let Some(initialize_client_name_tx) = initialize_client_name_tx.take()
            {
                let _ = initialize_client_name_tx.send(client_name);
            }
            if !forward_incoming_message(
                &transport_event_tx_for_reader,
                &writer_tx_for_reader,
                connection_id,
                &line,
            )
            .await
            {
                break;
            }
        }

        // EOF can finish the transport before RPC or runtime cleanup. Start
        // the same process deadline even if no SIGTERM arrives.
        if cfg!(unix) && install_shutdown_signal_handler {
            start_shutdown_watchdog();
        }
        let _ = transport_event_tx_for_reader
            .send(TransportEvent::ConnectionClosed { connection_id })
            .await;
        debug!("stdin reader finished (EOF)");
    };

    let write_messages = async move {
        while let Some(queued_message) = writer_rx.recv().await {
            let Some(mut json) = serialize_outgoing_message(queued_message.message) else {
                continue;
            };
            json.push('\n');
            let (written_tx, written_rx) = oneshot::channel();
            if stdout_tx.send((json, written_tx)).await.is_err() || written_rx.await.is_err() {
                break;
            }
            if let Some(write_complete_tx) = queued_message.write_complete_tx {
                let _ = write_complete_tx.send(());
            }
        }
        info!("stdout writer exited (channel closed)");
    };

    Ok(tokio::spawn(async move {
        tokio::select! {
            _ = shutdown_signal.cancelled() => {
                // Cancelling both forwarding futures drops their queues before
                // connection teardown, including when EOF already began draining.
                info!("SIGTERM received; closing stdio connection (45s shutdown deadline)");
                let _ = transport_event_tx
                    .send(TransportEvent::ConnectionClosed { connection_id })
                    .await;
            }
            _ = async move { tokio::join!(read_messages, write_messages); } => {}
        }
    }))
}

fn start_shutdown_watchdog() {
    // EOF and SIGTERM can both start cleanup; keep the first process deadline.
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        std::thread::Builder::new()
            .name("app-server-shutdown".to_string())
            .spawn(|| {
                // Allow the processor's 30s RPC drain, then bound even Tokio's
                // runtime teardown. Do not log here: stderr may also be blocked.
                std::thread::sleep(std::time::Duration::from_secs(45));
                std::process::exit(/*code*/ 1);
            })
            .unwrap_or_else(|_| std::process::exit(/*code*/ 1));
    });
}

fn stdio_initialize_client_name(line: &str) -> Option<String> {
    let message = serde_json::from_str::<JSONRPCMessage>(line).ok()?;
    let JSONRPCMessage::Request(JSONRPCRequest { method, params, .. }) = message else {
        return None;
    };
    if method != "initialize" {
        return None;
    }
    let params = serde_json::from_value::<InitializeParams>(params?).ok()?;
    Some(params.client_info.name)
}
