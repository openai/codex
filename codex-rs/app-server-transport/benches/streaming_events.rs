use codex_app_server_protocol::item_event_to_server_notification;
use codex_app_server_transport::OutgoingMessage;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandOutputDeltaEvent;
use codex_protocol::protocol::ExecOutputStream;
use divan::Bencher;
use divan::counter::BytesCount;
use divan::counter::ItemsCount;

const EVENTS_PER_BATCH: usize = 256;
const THREAD_ID: &str = "019ef47c-0000-7000-8000-000000000001";
const TURN_ID: &str = "019ef47c-0000-7000-8000-000000000002";
const ITEM_ID: &str = "item_019ef47c000070008000000000000003";

fn main() {
    divan::main();
}

#[divan::bench(args = [16, 256, 4096])]
fn project_and_serialize_agent_message_delta(bencher: Bencher, delta_bytes: usize) {
    // Input generation is excluded because the event already exists when the
    // app-server receives it from core.
    bencher
        .counter(ItemsCount::new(1_usize))
        .counter(BytesCount::new(delta_bytes))
        .with_inputs(|| {
            EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent {
                thread_id: THREAD_ID.to_string(),
                turn_id: TURN_ID.to_string(),
                item_id: ITEM_ID.to_string(),
                delta: "x".repeat(delta_bytes),
            })
        })
        .bench_local_values(|event| {
            let message = project_event(event);
            serialize_outgoing_message(&message).len()
        });
}

#[divan::bench(args = [64, 4096, 65536])]
fn project_and_serialize_command_output_delta(bencher: Bencher, chunk_bytes: usize) {
    // Input generation is excluded because the event already exists when the
    // app-server receives it from core.
    bencher
        .counter(ItemsCount::new(1_usize))
        .counter(BytesCount::new(chunk_bytes))
        .with_inputs(|| {
            EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                call_id: ITEM_ID.to_string(),
                stream: ExecOutputStream::Stdout,
                chunk: vec![b'x'; chunk_bytes],
            })
        })
        .bench_local_values(|event| {
            let message = project_event(event);
            serialize_outgoing_message(&message).len()
        });
}

/// Measures dispatch preparation and encoding after a streaming event has
/// already been projected. The dispatcher clones for all but the final
/// subscriber and moves the original message to the final queue.
#[divan::bench(args = [1, 4, 16])]
fn serialize_agent_message_delta_fanout(bencher: Bencher, subscriber_count: usize) {
    let message = project_event(EventMsg::AgentMessageContentDelta(
        AgentMessageContentDeltaEvent {
            thread_id: THREAD_ID.to_string(),
            turn_id: TURN_ID.to_string(),
            item_id: ITEM_ID.to_string(),
            delta: "streaming assistant output".to_string(),
        },
    ));
    let serialized_messages = EVENTS_PER_BATCH * subscriber_count;

    bencher
        .counter(ItemsCount::new(serialized_messages))
        .with_inputs(|| vec![message.clone(); EVENTS_PER_BATCH])
        .bench_local_values(|messages| {
            let mut encoded_bytes = 0;
            for message in messages {
                for _ in 1..subscriber_count {
                    let subscriber_message = message.clone();
                    encoded_bytes += serialize_outgoing_message(&subscriber_message).len();
                }
                encoded_bytes += serialize_outgoing_message(&message).len();
            }
            encoded_bytes
        });
}

fn project_event(event: EventMsg) -> OutgoingMessage {
    let notification = item_event_to_server_notification(event, THREAD_ID, TURN_ID);
    OutgoingMessage::AppServerNotification(notification)
}

fn serialize_outgoing_message(message: &OutgoingMessage) -> String {
    match serde_json::to_string(message) {
        Ok(json) => json,
        Err(error) => panic!("outgoing message should serialize: {error}"),
    }
}
