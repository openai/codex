use super::*;

async fn render_clear_ui_header_after_long_transcript_for_snapshot() -> String {
    let mut app = make_test_app().await;
    app.config.cwd = test_path_buf("/tmp/project").abs();
    app.chat_widget.set_model("gpt-test");
    app.chat_widget
        .set_reasoning_effort(Some(ReasoningEffortConfig::High));
    let story_part_one = "In the cliffside town of Bracken Ferry, the lighthouse had been dark for \
        nineteen years, and the children were told it was because the sea no longer wanted a \
        guide. Mara, who repaired clocks for a living, found that hard to believe. Every dawn she \
        heard the gulls circling the empty tower, and every dusk she watched ships hesitate at the \
        mouth of the bay as if listening for a signal that never came. When an old brass key fell \
        out of a cracked parcel in her workshop, tagged only with the words 'for the lamp room,' \
        she decided to climb the hill and see what the town had forgotten.";
    let story_part_two = "Inside the lighthouse she found gears wrapped in oilcloth, logbooks filled \
        with weather notes, and a lens shrouded beneath salt-stiff canvas. The mechanism was not \
        broken, only unfinished. Someone had removed the governor spring and hidden it in a false \
        drawer, along with a letter from the last keeper admitting he had darkened the light on \
        purpose after smugglers threatened his family. Mara spent the night rebuilding the clockwork \
        from spare watch parts, her fingers blackened with soot and grease, while a storm gathered \
        over the water and the harbor bells began to ring.";
    let story_part_three = "At midnight the first squall hit, and the fishing boats returned early, \
        blind in sheets of rain. Mara wound the mechanism, set the teeth by hand, and watched the \
        great lens begin to turn in slow, certain arcs. The beam swept across the bay, caught the \
        whitecaps, and reached the boats just as they were drifting toward the rocks below the \
        eastern cliffs. In the morning the town square was crowded with wet sailors, angry elders, \
        and wide-eyed children, but when the oldest captain placed the keeper's log on the fountain \
        and thanked Mara for relighting the coast, nobody argued. By sunset, Bracken Ferry had a \
        lighthouse again, and Mara had more clocks to mend than ever because everyone wanted \
        something in town to keep better time.";

    let user_cell = |text: &str| -> Arc<dyn HistoryCell> {
        Arc::new(UserHistoryCell {
            message: text.to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        }) as Arc<dyn HistoryCell>
    };
    let agent_cell = |text: &str| -> Arc<dyn HistoryCell> {
        Arc::new(AgentMessageCell::new(
            vec![Line::from(text.to_string())],
            /*is_first_line*/ true,
        )) as Arc<dyn HistoryCell>
    };
    let make_header = |is_first| -> Arc<dyn HistoryCell> {
        let event = SessionConfiguredEvent {
            session_id: ThreadId::new(),
            forked_from_id: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            cwd: test_path_buf("/tmp/project").abs(),
            reasoning_effort: Some(ReasoningEffortConfig::High),
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        };
        Arc::new(new_session_info(
            app.chat_widget.config_ref(),
            app.chat_widget.current_model(),
            event,
            is_first,
            /*tooltip_override*/ None,
            /*auth_plan*/ None,
            /*show_fast_status*/ false,
        )) as Arc<dyn HistoryCell>
    };

    app.transcript_cells = vec![
        make_header(/*is_first*/ true),
        Arc::new(crate::history_cell::new_info_event(
            "startup tip that used to replay".to_string(),
            /*hint*/ None,
        )) as Arc<dyn HistoryCell>,
        user_cell("Tell me a long story about a town with a dark lighthouse."),
        agent_cell(story_part_one),
        user_cell("Continue the story and reveal why the light went out."),
        agent_cell(story_part_two),
        user_cell("Finish the story with a storm and a resolution."),
        agent_cell(story_part_three),
    ];
    app.has_emitted_history_lines = true;

    let rendered = app
        .clear_ui_header_lines_with_version(/*width*/ 80, "<VERSION>")
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !rendered.contains("startup tip that used to replay"),
        "clear header should not replay startup notices"
    );
    assert!(
        !rendered.contains("Bracken Ferry"),
        "clear header should not replay prior conversation turns"
    );
    rendered
}

#[tokio::test]
#[cfg_attr(
    target_os = "windows",
    ignore = "snapshot path rendering differs on Windows"
)]
async fn clear_ui_after_long_transcript_snapshots_fresh_header_only() {
    let rendered = render_clear_ui_header_after_long_transcript_for_snapshot().await;
    assert_snapshot!("clear_ui_after_long_transcript_fresh_header_only", rendered);
}

#[tokio::test]
#[cfg_attr(
    target_os = "windows",
    ignore = "snapshot path rendering differs on Windows"
)]
async fn ctrl_l_clear_ui_after_long_transcript_reuses_clear_header_snapshot() {
    let rendered = render_clear_ui_header_after_long_transcript_for_snapshot().await;
    assert_snapshot!("clear_ui_after_long_transcript_fresh_header_only", rendered);
}

#[tokio::test]
#[cfg_attr(
    target_os = "windows",
    ignore = "snapshot path rendering differs on Windows"
)]
async fn clear_ui_header_shows_fast_status_for_fast_capable_models() {
    let mut app = make_test_app().await;
    app.config.cwd = test_path_buf("/tmp/project").abs();
    app.chat_widget.set_model("gpt-5.4");
    set_fast_mode_test_catalog(&mut app.chat_widget);
    app.chat_widget
        .set_reasoning_effort(Some(ReasoningEffortConfig::XHigh));
    app.chat_widget
        .set_service_tier(Some(codex_protocol::config_types::ServiceTier::Fast));
    set_chatgpt_auth(&mut app.chat_widget);
    set_fast_mode_test_catalog(&mut app.chat_widget);

    let rendered = app
        .clear_ui_header_lines_with_version(/*width*/ 80, "<VERSION>")
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert_snapshot!("clear_ui_header_fast_status_fast_capable_models", rendered);
}
