use super::*;

#[tokio::test]
async fn rejected_candidate_clears_pending_state_without_touching_active_runtime() {
    let (session, turn_context) = crate::session::tests::make_session_and_context().await;
    let active_manager = session.services.mcp_connection_manager.load_full();
    let active_startup_token = session.mcp_startup_cancellation_token().await;
    let candidate_startup_token = CancellationToken::new();

    {
        let pending = PendingMcpManager::new(
            &session.services.pending_mcp_connection_manager,
            McpConnectionManager::new_uninitialized_with_permission_profile(
                &turn_context.approval_policy,
                &turn_context.permission_profile(),
                turn_context.config.prefix_mcp_tool_names(),
            ),
            candidate_startup_token.clone(),
        );
        assert!(Arc::ptr_eq(
            &pending.manager,
            &session
                .services
                .pending_mcp_connection_manager
                .load_full()
                .expect("candidate manager should be pending")
        ));
    }

    assert!(
        session
            .services
            .pending_mcp_connection_manager
            .load_full()
            .is_none()
    );
    assert!(candidate_startup_token.is_cancelled());
    assert!(!active_startup_token.is_cancelled());
    assert!(Arc::ptr_eq(
        &active_manager,
        &session.services.mcp_connection_manager.load_full()
    ));
}
