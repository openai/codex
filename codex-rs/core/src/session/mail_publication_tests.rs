//! Mail delivery order and timestamp persistence in the recipient rollout.
#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn record_inter_agent_communication_sets_turn_id_in_rollout_and_resume() {
    let (mut session, turn_context) = make_session_and_context().await;
    let rollout_path = attach_thread_persistence(&mut session).await;
    let communication = InterAgentCommunication::new(
        AgentPath::root().join("worker").expect("worker path"),
        AgentPath::root(),
        Vec::new(),
        "child done".to_string(),
        /*trigger_turn*/ false,
    );
    let mut expected_item = communication.to_model_input_item();
    let mut expected = serde_json::to_value(&expected_item).unwrap();
    expected["internal_chat_message_metadata_passthrough"] = json!({
        "content_item_metadata": [{"harness_injected": true, "source_tool_namespace": "collaboration"}],
    });
    expected_item = serde_json::from_value(expected).unwrap();
    expected_item.set_turn_id_if_missing(&turn_context.sub_id);

    session
        .record_inter_agent_communication(&turn_context, turn_context.model_info(), communication)
        .await;

    let recorded_history = session.clone_history().await;
    let recorded_items = raw_history_items(&recorded_history);
    let create_time = recorded_items[0]
        .executed_tool_call_metadata()
        .and_then(|metadata| metadata.create_time.clone())
        .expect("locally authored agent message should receive a creation timestamp");
    assert!(create_time.as_f64().is_some_and(|seconds| seconds > 0.0));
    expected_item.set_create_time_if_missing(create_time);

    assert_eq!(
        strip_response_item_ids(&recorded_items),
        strip_response_item_ids(std::slice::from_ref(&expected_item))
    );

    session.flush_rollout().await.expect("rollout should flush");
    let InitialHistory::Resumed(resumed) = RolloutRecorder::get_rollout_history(&rollout_path)
        .await
        .expect("read rollout history")
    else {
        panic!("expected resumed rollout history");
    };
    let persisted_items = resumed
        .history
        .iter()
        .filter(|item| {
            matches!(
                item,
                RolloutItem::ResponseItem(_)
                    | RolloutItem::InterAgentCommunication(_)
                    | RolloutItem::InterAgentCommunicationMetadata { .. }
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let expected_persisted_items = vec![
        RolloutItem::InterAgentCommunicationMetadata {
            trigger_turn: false,
        },
        RolloutItem::ResponseItem(ResponseItemEnvelope {
            item: expected_item.clone(),
            metadata: Some(CodexHarnessMetadata {
                user_input_order: Some(0),
                history_sequence: Some(1),
                sampled_through_sequence: Some(0),
                ..Default::default()
            }),
        }),
    ];
    assert_eq!(
        strip_response_item_ids_from_json(serde_json::to_value(persisted_items).unwrap()),
        strip_response_item_ids_from_json(serde_json::to_value(expected_persisted_items).unwrap())
    );

    let (resumed_session, _resumed_turn_context) = make_session_and_context().await;
    resumed_session
        .record_initial_history(InitialHistory::Resumed(resumed))
        .await;
    assert_eq!(
        strip_response_item_ids(&raw_history_items(&resumed_session.clone_history().await)),
        strip_response_item_ids(std::slice::from_ref(&expected_item))
    );
    assert_eq!(resumed_session.reserve_user_input_order().await, 1);
}
