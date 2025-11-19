#![allow(clippy::expect_used)]
use anyhow::Result;
use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::SavedSessionEntry;
use codex_core::build_saved_session_entry;
use codex_core::config::Config;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::RolloutItem;
use codex_core::protocol::RolloutLine;
use codex_core::protocol::SessionSource;
use codex_core::resolve_saved_session;
use codex_core::upsert_saved_session;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;

fn completion_body(idx: usize, message: &str) -> String {
    let resp_id = format!("resp-{idx}");
    let msg_id = format!("msg-{idx}");
    sse(vec![
        ev_response_created(&resp_id),
        ev_assistant_message(&msg_id, message),
        ev_completed(&resp_id),
    ])
}

fn rollout_lines(path: &Path) -> Vec<RolloutLine> {
    let text = std::fs::read_to_string(path).expect("read rollout");
    text.lines()
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            let value: serde_json::Value = serde_json::from_str(line).expect("rollout line json");
            Some(serde_json::from_value::<RolloutLine>(value).expect("rollout line"))
        })
        .collect()
}

fn rollout_items_without_meta(path: &Path) -> Vec<RolloutItem> {
    rollout_lines(path)
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::SessionMeta(_) => None,
            other => Some(other),
        })
        .collect()
}

fn session_meta_count(path: &Path) -> usize {
    rollout_lines(path)
        .iter()
        .filter(|line| matches!(line.item, RolloutItem::SessionMeta(_)))
        .count()
}

async fn submit_text(codex: &Arc<CodexConversation>, text: &str) -> Result<()> {
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: text.to_string(),
            }],
        })
        .await?;
    let _ = wait_for_event(codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
    Ok(())
}

async fn save_session(
    name: &str,
    codex: &Arc<CodexConversation>,
    config: &Config,
    model: &str,
) -> Result<SavedSessionEntry> {
    codex.flush_rollout().await?;
    codex.set_session_name(Some(name.to_string())).await?;
    let entry =
        build_saved_session_entry(name.to_string(), codex.rollout_path(), model.to_string())
            .await?;
    upsert_saved_session(&config.codex_home, entry.clone()).await?;
    Ok(entry)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn save_and_resume_by_name() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    mount_sse_sequence(&server, vec![completion_body(1, "initial")]).await;

    let mut builder = test_codex();
    let initial = builder.build(&server).await?;
    submit_text(&initial.codex, "first turn").await?;

    let name = "alpha";
    let entry = save_session(
        name,
        &initial.codex,
        &initial.config,
        &initial.session_configured.model,
    )
    .await?;
    let resolved = resolve_saved_session(&initial.config.codex_home, name)
        .await?
        .expect("saved session");
    assert_eq!(entry, resolved);
    assert_eq!(session_meta_count(&entry.rollout_path), 1);

    let saved_items = rollout_items_without_meta(&entry.rollout_path);

    let resumed = builder
        .resume(&server, initial.home.clone(), entry.rollout_path.clone())
        .await?;
    assert_eq!(resumed.session_configured.session_id, entry.conversation_id);
    let resumed_items = rollout_items_without_meta(&resumed.session_configured.rollout_path);

    assert_eq!(
        serde_json::to_value(saved_items)?,
        serde_json::to_value(resumed_items)?
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn save_and_fork_by_name() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    mount_sse_sequence(&server, vec![completion_body(1, "base")]).await;

    let mut builder = test_codex();
    let initial = builder.build(&server).await?;
    submit_text(&initial.codex, "original").await?;

    let entry = save_session(
        "forkable",
        &initial.codex,
        &initial.config,
        &initial.session_configured.model,
    )
    .await?;

    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy"));
    let conversation_manager = ConversationManager::new(auth_manager.clone(), SessionSource::Exec);
    let forked = conversation_manager
        .fork_from_rollout(
            initial.config.clone(),
            entry.rollout_path.clone(),
            auth_manager,
        )
        .await?;

    assert_ne!(forked.session_configured.session_id, entry.conversation_id);
    assert_ne!(forked.conversation.rollout_path(), entry.rollout_path);
    assert_eq!(session_meta_count(&forked.conversation.rollout_path()), 1);

    let base_items = rollout_items_without_meta(&entry.rollout_path);
    let fork_items = rollout_items_without_meta(&forked.conversation.rollout_path());
    assert_eq!(
        serde_json::to_value(base_items)?,
        serde_json::to_value(fork_items)?
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forked_messages_do_not_touch_original() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    mount_sse_sequence(
        &server,
        vec![
            completion_body(1, "base"),
            completion_body(2, "fork-1"),
            completion_body(3, "fork-2"),
        ],
    )
    .await;

    let mut builder = test_codex();
    let initial = builder.build(&server).await?;
    submit_text(&initial.codex, "first").await?;

    let entry = save_session(
        "branch",
        &initial.codex,
        &initial.config,
        &initial.session_configured.model,
    )
    .await?;
    let baseline_items = rollout_items_without_meta(&entry.rollout_path);

    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy"));
    let conversation_manager = ConversationManager::new(auth_manager.clone(), SessionSource::Exec);
    let forked = conversation_manager
        .fork_from_rollout(
            initial.config.clone(),
            entry.rollout_path.clone(),
            auth_manager.clone(),
        )
        .await?;

    submit_text(&forked.conversation, "fork message one").await?;
    submit_text(&forked.conversation, "fork message two").await?;

    let resumed = builder
        .resume(&server, initial.home.clone(), entry.rollout_path.clone())
        .await?;
    let resumed_items = rollout_items_without_meta(&resumed.session_configured.rollout_path);

    assert_eq!(
        serde_json::to_value(baseline_items.clone())?,
        serde_json::to_value(resumed_items)?
    );
    assert_eq!(
        serde_json::to_value(baseline_items)?,
        serde_json::to_value(rollout_items_without_meta(&entry.rollout_path))?
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resumed_messages_are_present_in_new_fork() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    mount_sse_sequence(
        &server,
        vec![
            completion_body(1, "original"),
            completion_body(2, "fork-extra"),
            completion_body(3, "resumed-extra"),
        ],
    )
    .await;

    let mut builder = test_codex();
    let initial = builder.build(&server).await?;
    submit_text(&initial.codex, "start").await?;

    let entry = save_session(
        "seed",
        &initial.codex,
        &initial.config,
        &initial.session_configured.model,
    )
    .await?;

    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy"));
    let conversation_manager = ConversationManager::new(auth_manager.clone(), SessionSource::Exec);
    let forked = conversation_manager
        .fork_from_rollout(
            initial.config.clone(),
            entry.rollout_path.clone(),
            auth_manager.clone(),
        )
        .await?;
    submit_text(&forked.conversation, "fork only").await?;

    let resumed = builder
        .resume(&server, initial.home.clone(), entry.rollout_path.clone())
        .await?;
    submit_text(&resumed.codex, "resumed addition").await?;
    resumed.codex.flush_rollout().await?;
    let updated_base_items = rollout_items_without_meta(&entry.rollout_path);

    let fork_again = conversation_manager
        .fork_from_rollout(
            initial.config.clone(),
            entry.rollout_path.clone(),
            auth_manager,
        )
        .await?;
    let fork_again_items = rollout_items_without_meta(&fork_again.conversation.rollout_path());
    assert_eq!(
        serde_json::to_value(updated_base_items)?,
        serde_json::to_value(fork_again_items)?
    );
    assert_eq!(
        session_meta_count(&fork_again.conversation.rollout_path()),
        1
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn duplicate_name_overwrites_entry() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    mount_sse_sequence(
        &server,
        vec![completion_body(1, "one"), completion_body(2, "two")],
    )
    .await;

    let mut builder = test_codex();
    let first = builder.build(&server).await?;
    submit_text(&first.codex, "first session").await?;
    let name = "shared";
    let entry_one = save_session(
        name,
        &first.codex,
        &first.config,
        &first.session_configured.model,
    )
    .await?;

    let second = builder.build(&server).await?;
    submit_text(&second.codex, "second session").await?;
    let entry_two = save_session(
        name,
        &second.codex,
        &second.config,
        &second.session_configured.model,
    )
    .await?;

    let resolved = resolve_saved_session(&second.config.codex_home, name)
        .await?
        .expect("latest entry present");
    assert_eq!(resolved, entry_two);
    assert_ne!(resolved.conversation_id, entry_one.conversation_id);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_session_multiple_names() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    mount_sse_sequence(&server, vec![completion_body(1, "hello")]).await;

    let mut builder = test_codex();
    let session = builder.build(&server).await?;
    submit_text(&session.codex, "save twice").await?;

    let entry_first = save_session(
        "first",
        &session.codex,
        &session.config,
        &session.session_configured.model,
    )
    .await?;
    let entry_second = save_session(
        "second",
        &session.codex,
        &session.config,
        &session.session_configured.model,
    )
    .await?;

    let resolved_first = resolve_saved_session(&session.config.codex_home, "first")
        .await?
        .expect("first entry");
    let resolved_second = resolve_saved_session(&session.config.codex_home, "second")
        .await?
        .expect("second entry");

    assert_eq!(entry_first.conversation_id, entry_second.conversation_id);
    assert_eq!(
        resolved_first.conversation_id,
        resolved_second.conversation_id
    );
    assert_eq!(resolved_first.rollout_path, resolved_second.rollout_path);

    let names: serde_json::Value = json!([entry_first.name, entry_second.name]);
    assert_eq!(names, json!(["first", "second"]));

    Ok(())
}
