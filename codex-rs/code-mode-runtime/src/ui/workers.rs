//! Dedicated stdin, stdout, JS execution and watchdog threads. Queue admission is
//! per instance, and cancellation bypasses queued JS turns; V8 manages its own workers.

use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;

use super::engine::Engine;
use super::transport::Instance;
use super::transport::OUTPUT_SLOTS_PER_INSTANCE;
use super::transport::OutputReservation;
use super::transport::QueuedOutput;
use super::transport::Transport;
use super::transport::blocking_reader;
use super::transport::blocking_writer;
use super::watchdog::IsolateExecutionWatchdog;
use codex_plugin_host::MAX_INSTANCES;
use codex_plugin_host::protocol::ErrorCode;
use codex_plugin_host::protocol::HostMessage;
use codex_plugin_host::protocol::MAX_IN_FLIGHT_TURNS;
use codex_plugin_host::protocol::Packet;
use codex_plugin_host::protocol::RuntimeMessage;
use futures::SinkExt;
use futures::StreamExt;
use futures::executor::block_on;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;

// Every instance reserves space for its turns and one close; output also reserves
// one slot for a rejected open.
const COMMAND_QUEUE_CAPACITY: usize = MAX_INSTANCES * (MAX_IN_FLIGHT_TURNS + 1);
const OUTPUT_QUEUE_CAPACITY: usize = MAX_INSTANCES * (OUTPUT_SLOTS_PER_INSTANCE + 1) + 1;

enum Command {
    Turn {
        packet: Packet<HostMessage>,
        instance: Arc<Instance>,
        _permit: OwnedSemaphorePermit,
    },
    Close(Arc<Instance>),
}

pub(super) fn run() -> io::Result<()> {
    let (outgoing_tx, outgoing_rx) = mpsc::sync_channel::<QueuedOutput>(OUTPUT_QUEUE_CAPACITY);
    thread::Builder::new()
        .name("ui-plugin-output".into())
        .spawn(move || {
            let mut writer = blocking_writer(io::stdout().lock());
            if let Err(error) = block_on(writer.send(codex_plugin_host::READY.to_vec().into())) {
                fatal(error);
            }
            for output in outgoing_rx {
                // The parent can send a replacement as soon as it reads Closed.
                if let OutputReservation::Close { retired } = &output._reservation {
                    retired.store(true, Ordering::Release);
                }
                for packet in &output.packets {
                    let bytes =
                        serde_json::to_vec(packet).unwrap_or_else(|error| fatal(error.into()));
                    if let Err(error) = block_on(writer.send(bytes.into())) {
                        fatal(error);
                    }
                }
            }
        })?;
    let watchdog = IsolateExecutionWatchdog::new()?;
    let observer = watchdog.observer.clone();
    let (command_tx, command_rx) = mpsc::sync_channel(COMMAND_QUEUE_CAPACITY);
    let executor_outgoing_tx = outgoing_tx.clone();
    thread::Builder::new()
        .name("ui-plugin-executor".into())
        .spawn(move || {
            if let Err(error) = execute(command_rx, executor_outgoing_tx, watchdog) {
                fatal(error);
            }
        })?;
    let mut live = HashMap::<u64, (Arc<AtomicBool>, Weak<Instance>)>::new();
    let rejected_slot = Arc::new(Semaphore::new(1));
    let mut last_id = None;
    let mut input = blocking_reader(io::stdin().lock());
    loop {
        let packet: Packet<HostMessage> =
            serde_json::from_slice(&block_on(input.next()).ok_or(io::ErrorKind::UnexpectedEof)??)?;
        // An instance keeps its registration until its engine, queued turns and output drain.
        live.retain(|_, (retired, _)| !retired.load(Ordering::Acquire));
        let instance = match &packet.message {
            HostMessage::Initialize { .. } => {
                if last_id.is_some_and(|last| packet.instance <= last) {
                    return Err(io::Error::other("invalid plugin instance ID"));
                }
                last_id = Some(packet.instance);
                if live.len() >= MAX_INSTANCES {
                    // Backpressure excess opens instead of accumulating unbounded close frames.
                    let permit = block_on(Arc::clone(&rejected_slot).acquire_owned())
                        .map_err(io::Error::other)?;
                    outgoing_tx
                        .try_send(QueuedOutput {
                            packets: vec![close_packet(packet.instance)],
                            _reservation: OutputReservation::Rejected { _permit: permit },
                        })
                        .map_err(io::Error::other)?;
                    continue;
                }
                let instance = Arc::new(Instance::new(packet.instance));
                live.insert(
                    instance.id,
                    (Arc::clone(&instance.retired), Arc::downgrade(&instance)),
                );
                instance
            }
            HostMessage::Dispatch { .. } | HostMessage::Close => {
                let Some(instance) = live
                    .get(&packet.instance)
                    .and_then(|(_, instance)| instance.upgrade())
                else {
                    continue;
                };
                instance
            }
        };
        admit_turn(packet, instance, &observer, &command_tx)?;
    }
}

fn admit_turn(
    packet: Packet<HostMessage>,
    instance: Arc<Instance>,
    observer: &super::watchdog::Observer,
    command_tx: &mpsc::SyncSender<Command>,
) -> io::Result<()> {
    if instance.closed.load(Ordering::Acquire) {
        return Ok(());
    }
    let permit = if matches!(packet.message, HostMessage::Close) {
        None
    } else {
        Arc::clone(&instance.input_slots).try_acquire_owned().ok()
    };
    let command = if let Some(permit) = permit {
        Command::Turn {
            packet,
            instance,
            _permit: permit,
        }
    } else {
        if instance.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        observer.cancel(instance.id);
        Command::Close(instance)
    };
    command_tx.try_send(command).map_err(io::Error::other)
}

fn execute(
    command_rx: mpsc::Receiver<Command>,
    outgoing_tx: mpsc::SyncSender<QueuedOutput>,
    watchdog: IsolateExecutionWatchdog,
) -> io::Result<()> {
    let mut instances = HashMap::<u64, Engine>::new();
    for command in command_rx {
        let (packet, instance, _permit) = match command {
            Command::Close(instance) => {
                if instances.remove(&instance.id).is_some() {
                    send_close(&outgoing_tx, instance, /*error*/ None)?;
                }
                continue;
            }
            Command::Turn {
                packet,
                instance,
                _permit,
            } => (packet, instance, _permit),
        };
        if instance.closed.load(Ordering::Acquire) {
            if matches!(packet.message, HostMessage::Initialize { .. })
                || instances.remove(&instance.id).is_some()
            {
                send_close(&outgoing_tx, instance, /*error*/ None)?;
            }
            continue;
        }
        if matches!(packet.message, HostMessage::Initialize { .. }) {
            let transport = Transport {
                instance: Arc::clone(&instance),
                outgoing_tx: outgoing_tx.clone(),
            };
            match Engine::new(transport) {
                Ok(engine) => {
                    instances.insert(instance.id, engine);
                }
                Err(error) => {
                    eprintln!("plugin isolate initialization failed: {error}");
                    instance.closed.store(true, Ordering::Release);
                    send_close(&outgoing_tx, instance, /*error*/ None)?;
                    continue;
                }
            }
        }
        let Some(engine) = instances.get_mut(&instance.id) else {
            let was_closed = instance.closed.swap(true, Ordering::AcqRel);
            let error = (!was_closed).then_some(RuntimeMessage::Error {
                code: ErrorCode::ExecutionFailed,
                script_error: None,
            });
            send_close(&outgoing_tx, instance, error)?;
            continue;
        };
        watchdog.arm(instance.id, engine.handle());
        let result = engine
            .transport
            .ensure_open()
            .map(|()| engine.turn(packet.message));
        drop(_permit);
        let error = match (watchdog.disarm(), result) {
            (Ok(()), Ok(Ok(messages))) => {
                if engine.transport.write(messages).is_ok() {
                    continue;
                }
                None
            }
            (Ok(()), Ok(Err(error))) => Some(error),
            (Ok(()), Err(_)) => None,
            (Err(_), _) => Some(RuntimeMessage::Error {
                code: ErrorCode::ExecutionTimedOut,
                script_error: None,
            }),
        };
        let error = if instance.closed.swap(true, Ordering::AcqRel) {
            None
        } else {
            error
        };
        instances.remove(&instance.id);
        send_close(&outgoing_tx, instance, error)?;
    }
    Err(io::Error::other("plugin input stopped"))
}

fn close_packet(instance: u64) -> Packet<RuntimeMessage> {
    Packet {
        instance,
        message: RuntimeMessage::Closed,
    }
}

fn send_close(
    outgoing_tx: &mpsc::SyncSender<QueuedOutput>,
    instance: Arc<Instance>,
    error: Option<RuntimeMessage>,
) -> io::Result<()> {
    let mut packets = Vec::new();
    if let Some(error) = error {
        packets.push(Packet {
            instance: instance.id,
            message: error,
        });
    }
    packets.push(close_packet(instance.id));
    instance
        .close_output
        .set((
            outgoing_tx.clone(),
            QueuedOutput {
                packets,
                _reservation: OutputReservation::Close {
                    retired: Arc::clone(&instance.retired),
                },
            },
        ))
        .map_err(|_| io::Error::other("plugin close already scheduled"))
}

pub(super) fn fatal(error: io::Error) -> ! {
    eprintln!("plugin helper failed: {error}");
    std::process::exit(1);
}

#[cfg(test)]
#[path = "workers_tests.rs"]
mod tests;
