use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use rmcp::model::RequestId;

use super::ActiveTurnRegistry;

#[test]
fn colliding_request_id_strings_use_independent_turn_routes() {
    let active_turns = ActiveTurnRegistry::default();
    let thread_id = ThreadId::new();
    let numeric_request_id = RequestId::Number(7);
    let string_request_id = RequestId::String("7".into());
    let numeric_turn_id = "numeric-turn".to_string();
    let string_turn_id = "string-turn".to_string();

    active_turns.register(
        numeric_request_id.clone(),
        thread_id,
        numeric_turn_id.clone(),
    );
    active_turns.register(string_request_id.clone(), thread_id, string_turn_id.clone());

    let mut routed_request_ids = Vec::new();
    assert!(
        active_turns.with_request_id(thread_id, &numeric_turn_id, |request_id| {
            routed_request_ids.push(request_id.clone());
        })
    );
    assert!(
        active_turns.with_request_id(thread_id, &string_turn_id, |request_id| {
            routed_request_ids.push(request_id.clone());
        })
    );
    assert_eq!(
        routed_request_ids,
        vec![numeric_request_id.clone(), string_request_id.clone()]
    );

    active_turns.unregister(&numeric_request_id);

    assert!(!active_turns.with_request_id(thread_id, &numeric_turn_id, |_| {}));
    let mut remaining_request_id = None;
    assert!(
        active_turns.with_request_id(thread_id, &string_turn_id, |request_id| {
            remaining_request_id = Some(request_id.clone());
        })
    );
    assert_eq!(remaining_request_id, Some(string_request_id));
}

#[test]
fn reused_request_id_replaces_the_previous_turn_route() {
    let active_turns = ActiveTurnRegistry::default();
    let thread_id = ThreadId::new();
    let request_id = RequestId::Number(7);
    let first_turn_id = "first-turn".to_string();
    let second_turn_id = "second-turn".to_string();

    active_turns.register(request_id.clone(), thread_id, first_turn_id.clone());
    active_turns.register(request_id.clone(), thread_id, second_turn_id.clone());

    assert!(!active_turns.with_request_id(thread_id, &first_turn_id, |_| {}));
    let mut routed_request_id = None;
    assert!(
        active_turns.with_request_id(thread_id, &second_turn_id, |request_id| {
            routed_request_id = Some(request_id.clone());
        })
    );
    assert_eq!(routed_request_id, Some(request_id));
}
