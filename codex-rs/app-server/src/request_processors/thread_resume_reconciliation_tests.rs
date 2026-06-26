use super::super::thread_lifecycle::merge_turn_history_with_active_turn;
use super::super::thread_lifecycle::set_thread_status_and_interrupt_stale_turns;
use super::build_thread_resume_initial_turns_page;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadResumeInitialTurnsPageParams;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[test]
fn persisted_terminal_statuses_reject_same_id_in_progress_snapshot() {
    for persisted_status in [
        TurnStatus::Completed,
        TurnStatus::Interrupted,
        TurnStatus::Failed,
    ] {
        let older_turn = turn("older", TurnStatus::Completed, "older persisted item");
        let persisted_turn = turn("current", persisted_status, "authoritative persisted item");
        let stale_active_turn = turn("current", TurnStatus::InProgress, "stale active item");
        let mut turns = vec![older_turn.clone(), persisted_turn.clone()];

        let accepted = merge_turn_history_with_active_turn(&mut turns, stale_active_turn);

        assert_eq!(accepted, false);
        assert_eq!(turns, vec![older_turn, persisted_turn]);
    }
}

#[test]
fn current_snapshots_replace_or_append_without_duplicates() {
    struct Case {
        persisted_turns: Vec<Turn>,
        active_turn: Turn,
        expected_turns: Vec<Turn>,
    }

    let older_turn = turn("older", TurnStatus::Completed, "older");
    let persisted_partial = turn("current", TurnStatus::InProgress, "persisted partial");
    let active_partial = turn("current", TurnStatus::InProgress, "active partial");
    let active_completed = turn("current", TurnStatus::Completed, "active final");
    let active_interrupted = turn("current", TurnStatus::Interrupted, "active interrupted");
    let active_failed = turn("current", TurnStatus::Failed, "active failed");
    let next_active = turn("next", TurnStatus::InProgress, "next active");
    let cases = [
        Case {
            persisted_turns: Vec::new(),
            active_turn: active_partial.clone(),
            expected_turns: vec![active_partial.clone()],
        },
        Case {
            persisted_turns: vec![older_turn.clone(), persisted_partial.clone()],
            active_turn: active_partial.clone(),
            expected_turns: vec![older_turn.clone(), active_partial],
        },
        Case {
            persisted_turns: vec![older_turn.clone(), persisted_partial.clone()],
            active_turn: active_completed.clone(),
            expected_turns: vec![older_turn.clone(), active_completed],
        },
        Case {
            persisted_turns: vec![older_turn.clone(), persisted_partial.clone()],
            active_turn: active_interrupted.clone(),
            expected_turns: vec![older_turn.clone(), active_interrupted],
        },
        Case {
            persisted_turns: vec![older_turn.clone(), persisted_partial],
            active_turn: active_failed.clone(),
            expected_turns: vec![older_turn.clone(), active_failed],
        },
        Case {
            persisted_turns: vec![older_turn.clone()],
            active_turn: next_active.clone(),
            expected_turns: vec![older_turn, next_active],
        },
    ];

    for case in cases {
        let mut turns = case.persisted_turns;

        let accepted = merge_turn_history_with_active_turn(&mut turns, case.active_turn);

        assert_eq!(accepted, true);
        assert_eq!(turns, case.expected_turns);
        assert_eq!(
            turns
                .iter()
                .map(|turn| turn.id.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len(),
            turns.len(),
            "reconciliation must leave at most one snapshot for each turn id"
        );
    }
}

#[test]
fn thread_status_normalization_matrix() {
    struct Case {
        loaded_status: ThreadStatus,
        has_live_turn: bool,
        expected_status: ThreadStatus,
        expected_turn_status: TurnStatus,
    }

    let active = ThreadStatus::Active {
        active_flags: Vec::new(),
    };
    let waiting = ThreadStatus::Active {
        active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
    };
    let cases = [
        Case {
            loaded_status: ThreadStatus::Idle,
            has_live_turn: false,
            expected_status: ThreadStatus::Idle,
            expected_turn_status: TurnStatus::Interrupted,
        },
        Case {
            loaded_status: ThreadStatus::Idle,
            has_live_turn: true,
            expected_status: active.clone(),
            expected_turn_status: TurnStatus::InProgress,
        },
        Case {
            loaded_status: ThreadStatus::NotLoaded,
            has_live_turn: false,
            expected_status: ThreadStatus::NotLoaded,
            expected_turn_status: TurnStatus::Interrupted,
        },
        Case {
            loaded_status: ThreadStatus::NotLoaded,
            has_live_turn: true,
            expected_status: active,
            expected_turn_status: TurnStatus::InProgress,
        },
        Case {
            loaded_status: waiting.clone(),
            has_live_turn: false,
            expected_status: waiting.clone(),
            expected_turn_status: TurnStatus::InProgress,
        },
        Case {
            loaded_status: waiting.clone(),
            has_live_turn: true,
            expected_status: waiting,
            expected_turn_status: TurnStatus::InProgress,
        },
        Case {
            loaded_status: ThreadStatus::SystemError,
            has_live_turn: false,
            expected_status: ThreadStatus::SystemError,
            expected_turn_status: TurnStatus::Interrupted,
        },
        Case {
            loaded_status: ThreadStatus::SystemError,
            has_live_turn: true,
            expected_status: ThreadStatus::SystemError,
            expected_turn_status: TurnStatus::Interrupted,
        },
    ];

    for case in cases {
        let original_thread = thread(vec![turn("current", TurnStatus::InProgress, "active item")]);
        let mut thread = original_thread.clone();

        set_thread_status_and_interrupt_stale_turns(
            &mut thread,
            case.loaded_status,
            case.has_live_turn,
        );

        let mut expected_thread = original_thread;
        expected_thread.status = case.expected_status;
        expected_thread.turns[0].status = case.expected_turn_status;
        assert_eq!(thread, expected_thread);
    }
}

#[test]
fn persisted_terminal_turn_invalidates_stale_active_thread_status() {
    let history_items = completed_rollout("current", "persisted user", "persisted answer");
    let stale_active_turn = turn_with_user_and_agent_items(
        "current",
        TurnStatus::InProgress,
        "stale user",
        "stale partial",
    );

    let (resumed_thread, accepted_active_turn, has_live_turn) = project_full_resume(
        &history_items,
        Some(stale_active_turn),
        ThreadStatus::Active {
            active_flags: Vec::new(),
        },
        /*agent_running*/ true,
    );

    assert_eq!(accepted_active_turn, None);
    assert_eq!(has_live_turn, false);
    assert_eq!(resumed_thread.status, ThreadStatus::Idle);
    assert_eq!(resumed_thread.turns[0].status, TurnStatus::Completed);
}

#[test]
fn distinct_persisted_in_progress_turn_is_interrupted_when_new_turn_is_live() {
    let persisted_turn = turn(
        "incomplete-old-turn",
        TurnStatus::InProgress,
        "persisted partial",
    );
    let active_turn = turn("new-live-turn", TurnStatus::InProgress, "live partial");
    let mut turns = vec![persisted_turn.clone()];
    assert!(merge_turn_history_with_active_turn(
        &mut turns,
        active_turn.clone()
    ));
    let mut resumed_thread = thread(turns);

    set_thread_status_and_interrupt_stale_turns(
        &mut resumed_thread,
        ThreadStatus::Idle,
        /*has_live_in_progress_turn*/ true,
    );

    let mut expected_persisted_turn = persisted_turn;
    expected_persisted_turn.status = TurnStatus::Interrupted;
    let mut expected_thread = thread(vec![expected_persisted_turn, active_turn]);
    expected_thread.status = ThreadStatus::Active {
        active_flags: Vec::new(),
    };
    assert_eq!(resumed_thread, expected_thread);
    assert_eq!(
        resumed_thread
            .turns
            .iter()
            .filter(|turn| matches!(turn.status, TurnStatus::InProgress))
            .count(),
        1
    );
}

#[test]
fn full_turns_and_initial_page_apply_the_same_system_error_status() {
    let active_turn = turn("current", TurnStatus::InProgress, "live partial");
    let mut resumed_thread = thread(vec![active_turn.clone()]);
    set_thread_status_and_interrupt_stale_turns(
        &mut resumed_thread,
        ThreadStatus::SystemError,
        /*has_live_in_progress_turn*/ true,
    );

    let initial_page = build_thread_resume_initial_turns_page(
        &[],
        resumed_thread.status.clone(),
        /*has_live_running_thread*/ true,
        Some(active_turn),
        &full_ascending_page_params(),
    )
    .expect("initial turns page should build");

    assert_eq!(initial_page.data, resumed_thread.turns);
}

#[test]
fn full_turns_and_initial_page_interrupt_the_same_stale_in_progress_turn() {
    let history_items = incomplete_rollout("persisted", "persisted user", "persisted partial");
    let active_turn = turn_with_user_and_agent_items(
        "active",
        TurnStatus::InProgress,
        "active user",
        "active partial",
    );
    let (resumed_thread, accepted_active_turn, has_live_turn) = project_full_resume(
        &history_items,
        Some(active_turn),
        ThreadStatus::Idle,
        /*agent_running*/ true,
    );

    let initial_page = build_thread_resume_initial_turns_page(
        &history_items,
        resumed_thread.status.clone(),
        has_live_turn,
        accepted_active_turn,
        &full_ascending_page_params(),
    )
    .expect("initial turns page should build");

    assert_eq!(initial_page.data, resumed_thread.turns);
    assert_eq!(
        initial_page
            .data
            .iter()
            .filter(|turn| matches!(turn.status, TurnStatus::InProgress))
            .count(),
        1
    );
}

#[test]
fn full_turns_and_initial_page_share_ids_order_status_and_full_items() {
    let history_items = completed_rollout("persisted", "persisted user", "persisted answer");
    let active_turn = turn_with_user_and_agent_items(
        "active",
        TurnStatus::InProgress,
        "active user",
        "active partial",
    );
    let (resumed_thread, accepted_active_turn, has_live_turn) = project_full_resume(
        &history_items,
        Some(active_turn),
        ThreadStatus::Idle,
        /*agent_running*/ true,
    );

    let initial_page = build_thread_resume_initial_turns_page(
        &history_items,
        resumed_thread.status.clone(),
        has_live_turn,
        accepted_active_turn,
        &full_ascending_page_params(),
    )
    .expect("initial turns page should build");

    assert_eq!(initial_page.data, resumed_thread.turns);
}

#[test]
fn initial_page_item_views_do_not_change_turn_identity_or_status() {
    let active_turn = turn_with_user_and_agent_items(
        "active",
        TurnStatus::InProgress,
        "active user",
        "active partial",
    );

    for items_view in [
        TurnItemsView::NotLoaded,
        TurnItemsView::Summary,
        TurnItemsView::Full,
    ] {
        let page = build_thread_resume_initial_turns_page(
            &[],
            ThreadStatus::Active {
                active_flags: Vec::new(),
            },
            /*has_live_running_thread*/ true,
            Some(active_turn.clone()),
            &ThreadResumeInitialTurnsPageParams {
                limit: None,
                sort_direction: Some(SortDirection::Asc),
                items_view: Some(items_view),
            },
        )
        .expect("initial turns page should build");

        let mut expected_turn = active_turn.clone();
        match items_view {
            TurnItemsView::NotLoaded => expected_turn.items.clear(),
            TurnItemsView::Summary | TurnItemsView::Full => {}
        }
        expected_turn.items_view = items_view;
        assert_eq!(page.data, vec![expected_turn]);
    }
}

#[test]
fn stale_active_snapshot_is_excluded_from_both_full_turns_and_initial_page() {
    let history_items = completed_rollout("current", "persisted user", "persisted answer");
    let stale_active_turn = turn_with_user_and_agent_items(
        "current",
        TurnStatus::InProgress,
        "stale user",
        "stale partial",
    );
    let (resumed_thread, accepted_active_turn, has_live_turn) = project_full_resume(
        &history_items,
        Some(stale_active_turn),
        ThreadStatus::Idle,
        /*agent_running*/ true,
    );
    assert_eq!(accepted_active_turn, None);
    assert_eq!(has_live_turn, false);

    let initial_page = build_thread_resume_initial_turns_page(
        &history_items,
        resumed_thread.status.clone(),
        has_live_turn,
        accepted_active_turn,
        &full_ascending_page_params(),
    )
    .expect("initial turns page should build");

    assert_eq!(initial_page.data, resumed_thread.turns);
    assert_eq!(resumed_thread.status, ThreadStatus::Idle);
    assert_eq!(resumed_thread.turns[0].status, TurnStatus::Completed);
}

fn project_full_resume(
    history_items: &[RolloutItem],
    active_turn: Option<Turn>,
    loaded_status: ThreadStatus,
    agent_running: bool,
) -> (Thread, Option<Turn>, bool) {
    let mut turns = codex_app_server_protocol::build_turns_from_rollout_items(history_items);
    let active_turn_is_current = active_turn.as_ref().is_none_or(|active_turn| {
        merge_turn_history_with_active_turn(&mut turns, active_turn.clone())
    });
    let active_turn = active_turn.filter(|_| active_turn_is_current);
    let has_live_turn = active_turn_is_current
        && (agent_running
            || active_turn
                .as_ref()
                .is_some_and(|turn| matches!(turn.status, TurnStatus::InProgress)));
    let mut thread = thread(turns);
    set_thread_status_and_interrupt_stale_turns(&mut thread, loaded_status, has_live_turn);
    (thread, active_turn, has_live_turn)
}

fn full_ascending_page_params() -> ThreadResumeInitialTurnsPageParams {
    ThreadResumeInitialTurnsPageParams {
        limit: None,
        sort_direction: Some(SortDirection::Asc),
        items_view: Some(TurnItemsView::Full),
    }
}

fn turn(id: &str, status: TurnStatus, item_text: &str) -> Turn {
    Turn {
        id: id.to_string(),
        items: vec![ThreadItem::AgentMessage {
            id: format!("{id}-item"),
            text: item_text.to_string(),
            phase: None,
            memory_citation: None,
        }],
        items_view: TurnItemsView::Full,
        error: None,
        started_at: Some(1),
        completed_at: (!matches!(status, TurnStatus::InProgress)).then_some(2),
        duration_ms: (!matches!(status, TurnStatus::InProgress)).then_some(1_000),
        status,
    }
}

fn turn_with_user_and_agent_items(
    id: &str,
    status: TurnStatus,
    user_text: &str,
    agent_text: &str,
) -> Turn {
    Turn {
        id: id.to_string(),
        items: vec![
            ThreadItem::UserMessage {
                id: format!("{id}-user"),
                client_id: None,
                content: vec![UserInput::Text {
                    text: user_text.to_string(),
                    text_elements: Vec::new(),
                }],
            },
            ThreadItem::AgentMessage {
                id: format!("{id}-agent"),
                text: agent_text.to_string(),
                phase: None,
                memory_citation: None,
            },
        ],
        items_view: TurnItemsView::Full,
        error: None,
        started_at: Some(1),
        completed_at: (!matches!(status, TurnStatus::InProgress)).then_some(2),
        duration_ms: (!matches!(status, TurnStatus::InProgress)).then_some(1_000),
        status,
    }
}

fn completed_rollout(turn_id: &str, user_text: &str, agent_text: &str) -> Vec<RolloutItem> {
    [
        EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: turn_id.to_string(),
            trace_id: None,
            started_at: Some(1),
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        }),
        EventMsg::UserMessage(UserMessageEvent {
            client_id: None,
            message: user_text.to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
            ..Default::default()
        }),
        EventMsg::AgentMessage(AgentMessageEvent {
            message: agent_text.to_string(),
            phase: None,
            memory_citation: None,
        }),
        EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: turn_id.to_string(),
            last_agent_message: Some(agent_text.to_string()),
            completed_at: Some(2),
            duration_ms: Some(1_000),
            time_to_first_token_ms: None,
        }),
    ]
    .into_iter()
    .map(RolloutItem::EventMsg)
    .collect()
}

fn incomplete_rollout(turn_id: &str, user_text: &str, agent_text: &str) -> Vec<RolloutItem> {
    [
        EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: turn_id.to_string(),
            trace_id: None,
            started_at: Some(1),
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        }),
        EventMsg::UserMessage(UserMessageEvent {
            client_id: None,
            message: user_text.to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
            ..Default::default()
        }),
        EventMsg::AgentMessage(AgentMessageEvent {
            message: agent_text.to_string(),
            phase: None,
            memory_citation: None,
        }),
    ]
    .into_iter()
    .map(RolloutItem::EventMsg)
    .collect()
}

fn thread(turns: Vec<Turn>) -> Thread {
    Thread {
        id: "thread".to_string(),
        extra: None,
        session_id: "session".to_string(),
        forked_from_id: None,
        parent_thread_id: None,
        preview: String::new(),
        ephemeral: false,
        model_provider: "test".to_string(),
        created_at: 0,
        updated_at: 0,
        recency_at: None,
        status: ThreadStatus::Idle,
        path: None,
        cwd: AbsolutePathBuf::from_absolute_path(std::env::temp_dir())
            .expect("temporary directory should be absolute"),
        cli_version: "test".to_string(),
        source: SessionSource::Cli,
        thread_source: None,
        agent_nickname: None,
        agent_role: None,
        git_info: None,
        name: None,
        turns,
    }
}
