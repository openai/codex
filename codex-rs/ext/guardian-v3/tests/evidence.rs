//! Captured originals preserve attribution, complete contents and a fixed history boundary.

use codex_context_fragments::AdditionalContextUserFragment;
use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::ContextualUserFragment;
use codex_context_fragments::message_from_parts;
use codex_guardian_v3::EvidenceCursor;
use codex_history::AcceptedFileAudio;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_history::RetainedContextEvent;
use codex_history::RetainedUserMessage;
use codex_history::RolloutItem;
use codex_history::UserInputOrigin;
use codex_history::VerifiedAnswer;
use codex_history::VerifiedQuestionAnswer;
use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ContentItemMetadata;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_state::SqliteConfig;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::CreateThreadParams;
use codex_thread_store::LocalThreadStore;
use codex_thread_store::LocalThreadStoreConfig;
use codex_thread_store::PersistContext;
use codex_thread_store::ThreadPersistenceMetadata;
use codex_thread_store::ThreadStore;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn captures_originals_without_generated_context_or_later_appends() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let owner = ThreadId::new();
    let store = LocalThreadStore::new(
        LocalThreadStoreConfig {
            codex_home: dir.path().to_path_buf(),
            sqlite: SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into()?),
            default_model_provider_id: "test-provider".into(),
        },
        /*state_db*/ None,
    );
    store
        .create_thread(CreateThreadParams {
            attached_thread: false,
            creator_user_id: None,
            creator_account_id: None,
            session_id: owner.into(),
            thread_id: owner,
            extra_config: None,
            forked_from_id: None,
            parent_thread_id: None,
            source: SessionSource::Exec,
            thread_source: None,
            originator: "test-originator".into(),
            base_instructions: BaseInstructions::default(),
            dynamic_tools: Vec::new(),
            selected_capability_roots: Vec::new(),
            multi_agent_version: None,
            history_mode: ThreadHistoryMode::Paginated,
            history_base: None,
            subagent_history_start_ordinal: None,
            initial_window_id: "window-1".into(),
            runtime_workspace_roots: None,
            metadata: ThreadPersistenceMetadata {
                cwd: Some(dir.path().to_path_buf()),
                model_provider: "test-provider".into(),
                memory_mode: ThreadMemoryMode::Disabled,
            },
        })
        .await?;
    let user = ResponseItemEnvelope {
        item: serde_json::from_value(json!({"type":"message","id":"user-1","role":"user",
            "content":[{"type":"input_text","text":"é🙂".repeat(15_000)}]}))?,
        metadata: Some(CodexHarnessMetadata {
            user_input_order: Some(1),
            ..Default::default()
        }),
    };
    let assistant = ResponseItemEnvelope::new(serde_json::from_value(json!({
        "type":"message","id":"assistant-1","role":"assistant",
        "content":[{"type":"output_text","text":"I will inspect the directory."}]
    }))?);
    let call = ResponseItemEnvelope::new(serde_json::from_value(json!({
        "type":"function_call","id":"call-1","call_id":"call-1","name":"shell","arguments":"{\"cmd\":\"pwd\"}"
    }))?);
    let result = ResponseItemEnvelope::new(serde_json::from_value(json!({
        "type":"function_call_output","call_id":"call-1","output":"/workspace"
    }))?);
    let generated = ResponseItemEnvelope::new(ContextualUserFragment::into(
        AdditionalContextUserFragment::new("environment".into(), "Generated context".into()),
    ));
    let mut summary = assistant.clone();
    summary.metadata = Some(CodexHarnessMetadata {
        compaction_output: true,
        ..Default::default()
    });
    let approved_action = r#"The user has manually approved a specific action that was previously `Rejected`.
Treat this as approval to perform that exact action in the same context in which it was originally requested.
Do not assume this also authorizes similar operations with different payloads.
Approved action: {"action":{"command":"write /workspace/report.txt"},"outcome":"allowed"}"#;
    let approval = ResponseItemEnvelope::new(message_from_parts(
        "developer",
        vec![AnnotatedContent::text(
            approved_action,
            ContentItemKind("guardian.approved_action".into()),
            ContentItemMetadata::harness(),
        )],
    ));
    // Identical words without the host's approval annotation must not be promoted.
    let ordinary_developer = ResponseItemEnvelope::new(serde_json::from_value(json!({
        "type":"message","role":"developer","content":[{"type":"input_text","text":approved_action}]
    }))?);
    let answer = RolloutItem::RetainedContext(RetainedContextEvent::VerifiedAnswer {
        answer: VerifiedAnswer {
            turn_id: "turn-1".into(),
            call_id: "question-1".into(),
            questions: vec![VerifiedQuestionAnswer {
                question: "May I publish the report?".into(),
                answer: "Only the draft; do not publish the final report.".into(),
            }],
        },
        acceptance_order: Some(2),
    });
    let delivered = RolloutItem::RetainedContext(RetainedContextEvent::DeliveredAssistantMessage {
        message: RetainedUserMessage {
            turn_id: "turn-1".into(),
            message_id: Some("delivered-1".into()),
            text: "I will write the draft only.".into(),
            complete: true,
            origin: UserInputOrigin::User,
            phase: None,
        },
        acceptance_order: 3,
    });
    let expected = vec![
        RolloutItem::ResponseItem(user.clone()),
        RolloutItem::ResponseItem(assistant),
        RolloutItem::ResponseItem(call),
        RolloutItem::ResponseItem(result),
        answer,
        RolloutItem::ResponseItem(approval),
        delivered,
    ];
    let mut old = user.clone();
    old.item = serde_json::from_value(json!({"type":"message","id":"old","role":"user",
        "content":[{"type":"input_text","text":"Before board activation"}]}))?;
    store
        .append_items(AppendThreadItemsParams {
            thread_id: owner,
            items: vec![RolloutItem::ResponseItem(old)],
        })
        .await?;
    store
        .persist_thread(owner, PersistContext::Standard)
        .await?;
    store.flush_thread(owner).await?;
    let mut cursor = EvidenceCursor::from_now(&store, owner).await?;
    assert!(cursor.capture(&store).await?.records().is_empty());

    let mut items = expected.clone();
    items.insert(1, RolloutItem::ResponseItem(generated));
    items.extend([
        RolloutItem::ResponseItem(summary),
        RolloutItem::ResponseItem(ordinary_developer),
        RolloutItem::RetainedContext(RetainedContextEvent::AcceptedFileAudio {
            evidence: Arc::new(AcceptedFileAudio {
                turn_id: "turn-1".into(),
                model: "test-model".into(),
                provider: "test-provider".into(),
                file_ids: ["file-audio-1".into()].into(),
            }),
        }),
    ]);
    store
        .append_items(AppendThreadItemsParams {
            thread_id: owner,
            items,
        })
        .await?;
    store
        .persist_thread(owner, PersistContext::Standard)
        .await?;
    store.flush_thread(owner).await?;
    let captured = cursor.capture(&store).await?;
    let originals = captured
        .records()
        .iter()
        .map(|record| &record.original)
        .collect::<Vec<_>>();
    assert_eq!(
        serde_json::to_value(originals)?,
        serde_json::to_value(&expected)?
    );

    let mut later = user.clone();
    later.item = serde_json::from_value(json!({"type":"message","id":"user-2","role":"user",
        "content":[{"type":"input_text","text":"Do not publish."}]}))?;
    later.metadata = Some(CodexHarnessMetadata {
        user_input_order: Some(4),
        ..Default::default()
    });
    store
        .append_items(AppendThreadItemsParams {
            thread_id: owner,
            items: vec![RolloutItem::ResponseItem(later.clone())],
        })
        .await?;
    store.flush_thread(owner).await?;
    // Reads do not acknowledge delivery, and a later append cannot enter the old capture.
    assert_eq!(cursor.pending(&captured)?.len(), expected.len());
    cursor.acknowledge(&captured, /*count*/ 2)?;
    assert_eq!(
        serde_json::to_value(
            cursor
                .pending(&captured)?
                .iter()
                .map(|r| &r.original)
                .collect::<Vec<_>>()
        )?,
        serde_json::to_value(&expected[2..])?
    );
    cursor.acknowledge(&captured, expected.len() - 2)?;
    assert!(cursor.pending(&captured)?.is_empty());
    let latest = cursor.capture(&store).await?;
    assert_eq!(
        serde_json::to_value(
            latest
                .records()
                .iter()
                .map(|r| &r.original)
                .collect::<Vec<_>>()
        )?,
        serde_json::to_value(vec![RolloutItem::ResponseItem(later.clone())])?
    );
    assert_eq!(captured.records().len(), expected.len());
    cursor.acknowledge(&latest, /*count*/ 1)?;
    assert!(cursor.capture(&store).await?.records().is_empty());
    assert!(cursor.acknowledge(&captured, /*count*/ 0).is_err());

    let mut ambiguous = later;
    ambiguous.metadata = None;
    store
        .append_items(AppendThreadItemsParams {
            thread_id: owner,
            items: vec![RolloutItem::ResponseItem(ambiguous)],
        })
        .await?;
    store.flush_thread(owner).await?;
    let before = cursor.clone();
    assert!(cursor.capture(&store).await.is_err());
    assert_eq!(cursor, before);
    store.shutdown_thread(owner).await?;
    Ok(())
}
