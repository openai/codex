use super::*;
use pretty_assertions::assert_eq;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::NumberOrString;

fn test_elicitation() -> Elicitation {
    Elicitation::Mcp(CreateElicitationRequestParams::FormElicitationParams {
        meta: None,
        message: "Confirm?".to_string(),
        requested_schema: rmcp::model::ElicitationSchema::builder()
            .build()
            .expect("schema should build"),
    })
}

fn test_manager(state: McpElicitationState) -> ElicitationRequestManager {
    ElicitationRequestManager::new_with_state(
        AskForApproval::OnRequest,
        PermissionProfile::default(),
        /*reviewer*/ None,
        McpElicitationRuntimeMetadata::default(),
        state,
    )
}

#[tokio::test]
async fn failed_event_delivery_removes_the_response_route() {
    let state = McpElicitationState::default();
    let manager = test_manager(state.clone());
    let (tx_event, rx_event) = async_channel::bounded(1);
    drop(rx_event);

    let error = manager.make_sender("server".to_string(), tx_event)(
        NumberOrString::Number(7),
        test_elicitation(),
    )
    .await
    .expect_err("closed event channel must fail the elicitation");

    assert!(error.to_string().contains("failed to send MCP elicitation"));
    assert_eq!(state.requests.lock().expect("router lock").len(), 0);
}

#[tokio::test]
async fn cancelling_a_pending_elicitation_removes_the_response_route() {
    let state = McpElicitationState::default();
    let manager = test_manager(state.clone());
    let (tx_event, rx_event) = async_channel::unbounded();
    let task = tokio::spawn(manager.make_sender("server".to_string(), tx_event)(
        NumberOrString::Number(7),
        test_elicitation(),
    ));
    rx_event.recv().await.expect("elicitation request event");
    assert_eq!(state.requests.lock().expect("router lock").len(), 1);

    task.abort();
    assert!(
        task.await
            .expect_err("task must be cancelled")
            .is_cancelled()
    );
    assert_eq!(state.requests.lock().expect("router lock").len(), 0);
}
