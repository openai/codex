// New integration tests for web search flow (injected stream)

use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;
use codex_core::ConversationManager;
use codex_core::protocol::{InputItem, Op, ReviewDecision};

use core_test_support::wait_for_event;
use tempfile::TempDir;

#[tokio::test]
async fn web_search_request_triggers_approval() {
	unsafe { std::env::set_var("CODEX_RS_TEST_INJECT_WEB_SEARCH_REQUEST", "1"); }
	let tmp = TempDir::new().expect("tmp");
	let mut cfg = ConfigToml::default();
	cfg.tools = Some(codex_core::config::ToolsToml { web_search_request: Some(true) });
	let config = codex_core::config::Config::load_from_base_config_with_overrides(
		cfg,
		ConfigOverrides::default(),
		tmp.path().to_path_buf(),
	)
	.expect("config");
	let manager = ConversationManager::default();
	let new_conv = manager.new_conversation(config).await.expect("conv");
	let conv = new_conv.conversation;

	let _ = conv
		.submit(Op::UserInput {
			items: vec![InputItem::Text { text: "please search".to_string() }],
		})
		.await
		.expect("submit");

	let _ev = wait_for_event(&conv, |m| matches!(m, codex_core::protocol::EventMsg::ExecApprovalRequest(_))).await;
}

#[tokio::test]
async fn web_search_deny_returns_function_output() {
	unsafe { std::env::set_var("CODEX_RS_TEST_INJECT_WEB_SEARCH_REQUEST", "1"); }
	let tmp = TempDir::new().expect("tmp");
	let mut cfg = ConfigToml::default();
	cfg.tools = Some(codex_core::config::ToolsToml { web_search_request: Some(true) });
	let config = codex_core::config::Config::load_from_base_config_with_overrides(
		cfg,
		ConfigOverrides::default(),
		tmp.path().to_path_buf(),
	)
	.expect("config");
	let manager = ConversationManager::default();
	let new_conv = manager.new_conversation(config).await.expect("conv");
	let conv = new_conv.conversation;

	let sub_id = conv
		.submit(Op::UserInput {
			items: vec![InputItem::Text { text: "please search".to_string() }],
		})
		.await
		.expect("submit");

	let _ = wait_for_event(&conv, |m| matches!(m, codex_core::protocol::EventMsg::ExecApprovalRequest(_))).await;
	conv
		.submit(Op::ExecApproval { id: sub_id, decision: ReviewDecision::Denied })
		.await
		.expect("deny");
}

#[tokio::test]
async fn web_search_fallback_on_error_injects_hint() {
	unsafe { std::env::set_var("CODEX_RS_TEST_INJECT_WEB_SEARCH_REQUEST", "1"); }
	let tmp = TempDir::new().expect("tmp");
	let mut cfg = ConfigToml::default();
	cfg.tools = Some(codex_core::config::ToolsToml { web_search_request: Some(true) });
	let config = codex_core::config::Config::load_from_base_config_with_overrides(
		cfg,
		ConfigOverrides::default(),
		tmp.path().to_path_buf(),
	)
	.expect("config");
	let manager = ConversationManager::default();
	let new_conv = manager.new_conversation(config).await.expect("conv");
	let conv = new_conv.conversation;

	let sub_id = conv
		.submit(Op::UserInput {
			items: vec![InputItem::Text { text: "please search".to_string() }],
		})
		.await
		.expect("submit");

	let _ = wait_for_event(&conv, |m| matches!(m, codex_core::protocol::EventMsg::ExecApprovalRequest(_))).await;
	conv
		.submit(Op::ExecApproval { id: sub_id, decision: ReviewDecision::Approved })
		.await
		.expect("approve");
} 