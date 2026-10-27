#![cfg(test)]
//! Exercise executor invariants that the stdin reader normally enforces.

use super::*;
use codex_plugin_host::protocol::ReturnMode;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn missing_engine_fails_only_its_instance() {
    crate::v8_init::ensure_v8_initialized().unwrap();
    let peer = Arc::new(Instance::new(/*id*/ 1));
    let missing = Arc::new(Instance::new(/*id*/ 2));
    let dispatch = || HostMessage::Dispatch {
        id: 0,
        export: "default".into(),
        return_mode: ReturnMode::Sync,
    };
    let (command_tx, command_rx) = mpsc::channel();
    for (instance, message) in [
        (
            &peer,
            HostMessage::Initialize {
                source: "export default () => 'alive'".into(),
            },
        ),
        (&missing, dispatch()),
        (&peer, dispatch()),
    ] {
        command_tx
            .send(Command::Turn {
                packet: Packet {
                    instance: instance.id,
                    message,
                },
                instance: Arc::clone(instance),
                _permit: Arc::clone(&instance.input_slots)
                    .try_acquire_owned()
                    .unwrap(),
            })
            .unwrap();
    }
    drop(command_tx);
    let (outgoing_tx, outgoing_rx) = mpsc::sync_channel(OUTPUT_QUEUE_CAPACITY);
    let error = execute(
        command_rx,
        outgoing_tx,
        IsolateExecutionWatchdog::new().unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "plugin input stopped");
    assert!(missing.closed.load(Ordering::Acquire));
    drop(missing);
    let packets: Vec<_> = outgoing_rx
        .into_iter()
        .flat_map(|output| output.packets)
        .collect();
    assert_eq!(
        serde_json::to_value(packets).unwrap(),
        json!([
            {"instance":1,"message":{"type":"initialized"}},
            {"instance":1,"message":{"type":"reply","id":0,"value":"alive"}},
            {"instance":2,"message":{"type":"error","code":"execution_failed","scriptError":null}},
            {"instance":2,"message":{"type":"closed"}}
        ])
    );
}

#[test]
fn close_waits_for_the_last_queued_turn() {
    let (outgoing_tx, outgoing_rx) = mpsc::sync_channel(OUTPUT_QUEUE_CAPACITY);
    let instance = Arc::new(Instance::new(/*id*/ 1));
    let queued_turn = Arc::clone(&instance);
    send_close(&outgoing_tx, instance, /*error*/ None).unwrap();
    assert!(matches!(
        outgoing_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    drop(queued_turn);
    let output = outgoing_rx.try_recv().unwrap();
    assert_eq!(
        serde_json::to_value(output.packets).unwrap(),
        json!([{"instance":1,"message":{"type":"closed"}}])
    );
}

#[test]
fn input_overload_closes_only_the_busy_instance() {
    let busy = Arc::new(Instance::new(/*id*/ 1));
    let peer = Arc::new(Instance::new(/*id*/ 2));
    let watchdog = IsolateExecutionWatchdog::new().unwrap();
    let (command_tx, command_rx) = mpsc::sync_channel(COMMAND_QUEUE_CAPACITY);
    let dispatch = |instance: &Arc<Instance>, id| {
        let packet = Packet {
            instance: instance.id,
            message: HostMessage::Dispatch {
                id,
                export: "run".into(),
                return_mode: ReturnMode::Sync,
            },
        };
        admit_turn(
            packet,
            Arc::clone(instance),
            &watchdog.observer,
            &command_tx,
        )
        .unwrap();
    };
    // Leave every turn queued so its permit stays in flight.
    for id in 0..=MAX_IN_FLIGHT_TURNS as u64 {
        dispatch(&busy, id);
    }
    dispatch(&peer, 0);
    for _ in 0..MAX_IN_FLIGHT_TURNS {
        assert!(
            matches!(command_rx.try_recv(), Ok(Command::Turn { instance, .. }) if instance.id == busy.id)
        );
    }
    assert!(
        matches!(command_rx.try_recv(), Ok(Command::Close(instance)) if instance.id == busy.id)
    );
    assert!(
        matches!(command_rx.try_recv(), Ok(Command::Turn { instance, .. }) if instance.id == peer.id)
    );
}
