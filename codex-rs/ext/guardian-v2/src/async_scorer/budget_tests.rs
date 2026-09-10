//! Contributor integration tests use the normal catalog-backed lifecycle setup.

use super::*;
use crate::async_scorer::authorization::ScoreAuthorization;
use pretty_assertions::assert_eq;

#[derive(Clone, Copy)]
enum BudgetOutcome {
    Fits,
    RequiresSync,
}

async fn catalog_budget_fixture(base_url: String, window: i64) -> Result<GuardianFailureFixture> {
    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_model_info_override(MODEL, move |model| {
            model.context_window = Some(window);
            model.effective_context_window_percent = 50;
            model.comp_hash = Some("budget-checkpoint".to_owned());
        })
        .with_model_info_override("gpt-5.5", |model| {
            model.comp_hash = Some("budget-checkpoint".to_owned());
        })
        .with_pre_build_hook(|home| {
            std::fs::write(home.join("config.toml"),
                "[features.guardianv2]\nenabled = true\nmax_parent_compaction_tokens = 4096\n[features.guardianv2.review_scope]\ncomputer_use_only = false\n[features.guardianv2.transcript]\ninclude_images = true\n").unwrap();
        })
        .with_config(move |config| {
            // A smaller parent window must not replace Luna's catalog allowance.
            config.model_context_window = Some(1_000);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.guardian_policy_config = Some(TEST_GUARDIAN_POLICY.to_owned());
            config.model_provider.base_url = Some(base_url);
        })
        .build_with_auto_env(&server).await?;
    let mut builder = ExtensionRegistryBuilder::new();
    super::super::install(
        &mut builder,
        test.thread_manager.auth_manager(),
        Arc::downgrade(&test.thread_manager),
    );
    let registry = builder.build();
    let session_store = ExtensionData::new("session-1");
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &test.config,
            session_source: &SessionSource::Exec,
            persistent_thread_state_available: false,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store: test.codex.thread_extension_data(),
        })
        .await;
    Ok(GuardianFailureFixture {
        test,
        registry,
        session_store,
    })
}

#[derive(Clone, Copy)]
enum BudgetEvidence {
    Checkpoint,
    Image,
    UserInstructions,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_accounts_for_parent_checkpoint() -> Result<()> {
    skip_if_no_network!(Ok(()));
    assert_catalog_budget(BudgetEvidence::Checkpoint).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_preserves_optional_evidence_or_defers_to_sync() -> Result<()> {
    skip_if_no_network!(Ok(()));
    assert_catalog_budget(BudgetEvidence::Image).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_preserves_complete_user_instructions_or_defers_to_sync() -> Result<()> {
    skip_if_no_network!(Ok(()));
    assert_catalog_budget(BudgetEvidence::UserInstructions).await
}

async fn assert_catalog_budget(evidence: BudgetEvidence) -> Result<()> {
    let windows = match evidence {
        BudgetEvidence::Checkpoint => [18_000, 32_000],
        // In the smaller window the text fits, but the 10K image reservation cannot.
        BudgetEvidence::Image => [14_000, 40_000],
        BudgetEvidence::UserInstructions => [18_000, 50_000],
    };
    for (window, outcome) in windows
        .into_iter()
        .zip([BudgetOutcome::RequiresSync, BudgetOutcome::Fits])
    {
        let server = responses::start_mock_server().await;
        let fixture = catalog_budget_fixture(server.uri(), window).await?;
        let response = responses::mount_sse_once(
            &server,
            responses::sse(vec![
                ev_assistant_message("budget-score", "low"),
                ev_completed("budget-score"),
            ]),
        )
        .await;
        let instruction = if matches!(evidence, BudgetEvidence::UserInstructions) {
            let padding = "é🙂 ".repeat(/*n*/ 3_500);
            format!("{padding}You may edit files only in the scratch directory.{padding}")
        } else {
            "Inspect README.md; do not change files.".to_owned()
        };
        let restriction = "Revoke permission to edit files. Only read README.md.";
        let approval = format!(
            "{}\nApproved action: read_file README.md",
            codex_guardian_context::MANUAL_APPROVAL_DEVELOPER_PREFIX
        );
        let mut user = user_instruction(&instruction);
        let (checkpoint, commentary) = match evidence {
            BudgetEvidence::Checkpoint => (
                Some(ResponseItem::ContextCompaction {
                    id: Some(ResponseItemId::from_server("cmp_budget".to_owned())),
                    encrypted_content: Some("checkpoint".repeat(/*n*/ 1_200)),
                    internal_chat_message_metadata_passthrough: None,
                }),
                (0..4)
                    .map(|index| {
                        format!(
                            "commentary {index}: {}",
                            "optional checkpoint commentary ".repeat(/*n*/ 180)
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
            BudgetEvidence::Image => {
                let ResponseItem::Message { content, .. } = &mut user else {
                    unreachable!("user instruction is a message");
                };
                content.push(ContentItem::InputImage {
                    image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGPgEpEDAABoAD1UCKP3AAAAAElFTkSuQmCC".to_owned(),
                    detail: None,
                });
                (None, vec!["optional old commentary ".repeat(/*n*/ 500)])
            }
            BudgetEvidence::UserInstructions => (None, Vec::new()),
        };
        let mut history = checkpoint.iter().cloned().collect::<Vec<_>>();
        history.push(user);
        if matches!(evidence, BudgetEvidence::UserInstructions) {
            history.push(ResponseItem::Message {
                id: None,
                role: "developer".to_owned(),
                content: vec![ContentItem::InputText {
                    text: approval.clone(),
                }],
                phase: None,
                internal_chat_message_metadata_passthrough: None,
            });
            history.push(user_instruction(restriction));
        }
        history.extend(commentary.into_iter().map(|text| ResponseItem::Message {
            id: None,
            role: "assistant".to_owned(),
            content: vec![ContentItem::InputText { text }],
            phase: Some(MessagePhase::Commentary),
            internal_chat_message_metadata_passthrough: None,
        }));
        fixture
            .test
            .codex
            .inject_response_items(history.clone())
            .await?;
        let history: Arc<dyn ConversationHistorySnapshot> = match evidence {
            BudgetEvidence::Checkpoint => Arc::new(TestRetainedHistory {
                retained: history.clone(),
                current: TestConversationHistory(history),
                compaction_model_hash: Some("budget-checkpoint".to_owned()),
            }),
            BudgetEvidence::Image | BudgetEvidence::UserInstructions => {
                fixture.test.codex.conversation_history_snapshot().await
            }
        };
        let thread_store = fixture.test.codex.thread_extension_data();
        if matches!(evidence, BudgetEvidence::UserInstructions) {
            thread_store.insert(SecurityRiskScore {
                scores: BTreeMap::from([("action_risk".to_owned(), 0.25)]),
                call_id: None,
                action: None,
                sampled_at: None,
            });
            let progress = thread_store.get::<GuardianV2ScoreProgress>().unwrap();
            let authorization = ScoreAuthorization::current(&fixture.test.codex).await;
            *progress.authorization.lock().unwrap() = Some(authorization);
            assert_eq!(
                cached_approval(
                    &fixture.registry,
                    thread_store,
                    "review action",
                    /*metrics*/ None
                )
                .await,
                Some(ReviewDecision::Approved)
            );
        }
        let turn_store = ExtensionData::new("turn-1");
        let tool_name = ToolName::plain("read_file");
        let payload = ToolPayload::Function {
            arguments: r#"{"path":"README.md"}"#.to_owned(),
        };
        fixture.registry.tool_lifecycle_contributors()[0]
            .on_tool_start(ToolStartInput {
                session_store: &fixture.session_store,
                thread_store,
                turn_store: &turn_store,
                turn_id: "turn-1",
                root_turn_id: None,
                call_id: "budget-call",
                originating_item_id: None,
                tool_name: &tool_name,
                mcp_tool: None,
                payload: &payload,
                conversation_history: history,
                source: ToolCallSource::Direct,
            })
            .await;
        if matches!(outcome, BudgetOutcome::RequiresSync) {
            fixture.assert_fails_closed("elevated_risk").await?;
            assert!(
                server
                    .received_requests()
                    .await
                    .unwrap()
                    .iter()
                    .all(|request| request.method.as_str() != "POST")
            );
            fixture.test.codex.shutdown_and_wait().await?;
            continue;
        }
        let progress = thread_store.get::<GuardianV2ScoreProgress>().unwrap();
        tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
            while progress.latest_scored_tool_call.load(Ordering::Acquire)
                < progress.latest_tool_call.load(Ordering::Acquire)
            {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        let request = response.single_request();
        let text = request.message_input_texts("user").concat();
        let request = request.body_json();
        let input = request["input"].to_string();
        if let Some(checkpoint) = checkpoint {
            assert_eq!(request["input"][2], serde_json::to_value(checkpoint)?);
            assert!(input.contains("commentary 0:"));
        } else if matches!(evidence, BudgetEvidence::Image) {
            assert!(input.contains("input_image"));
            assert!(input.contains("optional old commentary"));
        } else {
            assert!(text.contains(&format!(
                "[1] user: {instruction}\n[2] developer: {approval}\n[3] user: {restriction}\n"
            )));
        }
        assert!(text.contains(&instruction));
        assert!(input.contains("read_file"));
        assert_eq!(
            cached_approval(
                &fixture.registry,
                thread_store,
                "review action",
                /*metrics*/ None
            )
            .await,
            Some(ReviewDecision::Approved)
        );
        fixture.test.codex.shutdown_and_wait().await?;
    }
    Ok(())
}
