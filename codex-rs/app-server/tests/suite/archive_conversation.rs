use std::path::Path;

use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::ArchiveConversationParams;
use codex_app_server_protocol::ArchiveConversationResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::RequestId;
use codex_core::ARCHIVED_SESSIONS_SUBDIR;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_conversation_moves_rollout_into_archived_directory() {
    let codex_home = TempDir::new().expect("create temp dir");
    create_config_toml(codex_home.path()).expect("write config.toml");

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("initialize timeout")
        .expect("initialize request");

    let new_request_id = mcp
        .send_new_conversation_request(NewConversationParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await
        .expect("send newConversation");
    let new_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_request_id)),
    )
    .await
    .expect("newConversation timeout")
    .expect("newConversation response");

    let NewConversationResponse {
        conversation_id,
        rollout_path,
        ..
    } = to_response::<NewConversationResponse>(new_response)
        .expect("deserialize newConversation response");

    assert!(
        rollout_path.exists(),
        "expected rollout path {} to exist",
        rollout_path.display()
    );

    let archive_request_id = mcp
        .send_archive_conversation_request(ArchiveConversationParams {
            conversation_id,
            rollout_path: rollout_path.clone(),
        })
        .await
        .expect("send archiveConversation");
    let archive_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(archive_request_id)),
    )
    .await
    .expect("archiveConversation timeout")
    .expect("archiveConversation response");

    let _: ArchiveConversationResponse =
        to_response::<ArchiveConversationResponse>(archive_response)
            .expect("deserialize archiveConversation response");

    let archived_directory = codex_home.path().join(ARCHIVED_SESSIONS_SUBDIR);
    let archived_rollout_path =
        archived_directory.join(rollout_path.file_name().unwrap_or_else(|| {
            panic!("rollout path {} missing file name", rollout_path.display())
        }));

    assert!(
        !rollout_path.exists(),
        "expected rollout path {} to be moved",
        rollout_path.display()
    );
    assert!(
        archived_rollout_path.exists(),
        "expected archived rollout path {} to exist",
        archived_rollout_path.display()
    );
}

fn create_config_toml(codex_home: &Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(config_toml, config_contents())
}

fn config_contents() -> &'static str {
    r#"model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
"#
}
