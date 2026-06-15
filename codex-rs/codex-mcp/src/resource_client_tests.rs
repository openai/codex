use std::sync::Arc;

use arc_swap::ArcSwap;
use codex_config::Constrained;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;

use super::McpResourceClient;
use crate::McpConnectionManager;

#[test]
fn cache_key_changes_when_the_published_manager_changes() {
    let manager = Arc::new(ArcSwap::from_pointee(test_manager()));
    let client = McpResourceClient::new(Arc::clone(&manager));

    let initial_key = client.cache_key();
    assert_eq!(initial_key, client.cache_key());
    manager.store(manager.load_full());
    assert_eq!(initial_key, client.cache_key());

    manager.store(Arc::new(test_manager()));
    assert_ne!(initial_key, client.cache_key());
}

fn test_manager() -> McpConnectionManager {
    McpConnectionManager::new_uninitialized_with_permission_profile(
        &Constrained::allow_any(AskForApproval::OnRequest),
        &PermissionProfile::default(),
        /*prefix_mcp_tool_names*/ false,
    )
}
