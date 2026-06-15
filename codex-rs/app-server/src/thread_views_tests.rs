use super::*;

use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::ThreadSource as ApiThreadSource;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ThreadSource;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use uuid::Uuid;

#[test]
fn source_filter_defaults_to_interactive_sources() {
    for source_kinds in [None, Some(Vec::new())] {
        let filter = ThreadSourceFilter::new(source_kinds);

        assert_eq!(
            filter.store_sources(),
            INTERACTIVE_SESSION_SOURCES.as_slice()
        );
        assert!(filter.matches(&SessionSource::Cli));
        assert!(filter.matches(&SessionSource::Exec));
    }
}

#[test]
fn source_filter_uses_store_filter_for_interactive_kinds() {
    let filter =
        ThreadSourceFilter::new(Some(vec![ThreadSourceKind::Cli, ThreadSourceKind::VsCode]));

    assert_eq!(
        filter.store_sources(),
        &[SessionSource::Cli, SessionSource::VSCode]
    );
    assert!(filter.matches(&SessionSource::Cli));
    assert!(!filter.matches(&SessionSource::Exec));
}

#[test]
fn source_filter_distinguishes_subagent_variants() {
    let parent_thread_id =
        ThreadId::from_string(&Uuid::new_v4().to_string()).expect("valid thread id");
    let review = SessionSource::SubAgent(SubAgentSource::Review);
    let spawn = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id,
        depth: 1,
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
    });
    let review_filter = ThreadSourceFilter::new(Some(vec![ThreadSourceKind::SubAgentReview]));

    assert_eq!(review_filter.store_sources(), &[]);
    assert!(review_filter.matches(&review));
    assert!(!review_filter.matches(&spawn));
}

#[test]
fn stored_thread_projection_applies_fallbacks() {
    let created_at = DateTime::parse_from_rfc3339("2025-01-02T03:04:05Z")
        .expect("valid timestamp")
        .with_timezone(&Utc);
    let updated_at = DateTime::parse_from_rfc3339("2025-01-02T03:05:06Z")
        .expect("valid timestamp")
        .with_timezone(&Utc);
    let thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000123").expect("valid thread id");
    let cwd = PathBuf::from("/tmp/project");
    let fallback_cwd =
        AbsolutePathBuf::from_absolute_path(PathBuf::from("/tmp/fallback")).expect("absolute path");
    let stored_thread = StoredThread {
        thread_id,
        extra_config: None,
        rollout_path: None,
        forked_from_id: None,
        parent_thread_id: None,
        preview: "preview".to_string(),
        name: Some("name".to_string()),
        model_provider: String::new(),
        model: None,
        reasoning_effort: None,
        created_at,
        updated_at,
        archived_at: None,
        cwd: cwd.clone(),
        cli_version: "0.0.0".to_string(),
        source: SessionSource::Cli,
        thread_source: Some(ThreadSource::User),
        agent_nickname: None,
        agent_role: None,
        agent_path: None,
        git_info: None,
        approval_mode: AskForApproval::OnRequest,
        permission_profile: PermissionProfile::read_only(),
        token_usage: None,
        first_user_message: Some("preview".to_string()),
        history: None,
    };

    assert_eq!(
        from_stored_thread(stored_thread, "fallback-provider", &fallback_cwd),
        Thread {
            id: thread_id.to_string(),
            extra: None,
            session_id: thread_id.to_string(),
            forked_from_id: None,
            parent_thread_id: None,
            preview: "preview".to_string(),
            ephemeral: false,
            model_provider: "fallback-provider".to_string(),
            created_at: created_at.timestamp(),
            updated_at: updated_at.timestamp(),
            status: ThreadStatus::NotLoaded,
            path: None,
            cwd: AbsolutePathBuf::from_absolute_path(cwd).expect("absolute path"),
            cli_version: "0.0.0".to_string(),
            agent_nickname: None,
            agent_role: None,
            source: codex_app_server_protocol::SessionSource::Cli,
            thread_source: Some(ApiThreadSource::User),
            git_info: None,
            name: Some("name".to_string()),
            turns: Vec::new(),
        }
    );
}

#[test]
fn loaded_thread_pagination_sorts_and_excludes_anchor() {
    let first = "00000000-0000-0000-0000-000000000001".to_string();
    let second = "00000000-0000-0000-0000-000000000002".to_string();
    let third = "00000000-0000-0000-0000-000000000003".to_string();

    let first_page = paginate_loaded_thread_ids(
        vec![third.clone(), first.clone(), second.clone()],
        /*cursor*/ None,
        Some(2),
    )
    .expect("first page");
    assert_eq!(
        first_page,
        ThreadLoadedListResponse {
            data: vec![first, second.clone()],
            next_cursor: Some(second.clone()),
        }
    );

    assert_eq!(
        paginate_loaded_thread_ids(vec![third.clone(), second.clone()], Some(&second), Some(2))
            .expect("second page"),
        ThreadLoadedListResponse {
            data: vec![third],
            next_cursor: None,
        }
    );
}

#[test]
fn turn_pagination_preserves_order_across_pages() {
    let turns = ["turn-1", "turn-2", "turn-3"]
        .into_iter()
        .map(turn)
        .collect::<Vec<_>>();
    let first_page = paginate_turns(
        turns.clone(),
        /*cursor*/ None,
        Some(2),
        SortDirection::Desc,
        TurnItemsView::Full,
    )
    .expect("first page");
    assert_eq!(first_page.data, vec![turn("turn-3"), turn("turn-2")]);
    let next_cursor = first_page.next_cursor.expect("next cursor");

    let second_page = paginate_turns(
        turns,
        Some(&next_cursor),
        Some(2),
        SortDirection::Desc,
        TurnItemsView::Full,
    )
    .expect("second page");
    assert_eq!(second_page.data, vec![turn("turn-1")]);
    assert_eq!(second_page.next_cursor, None);
}

fn turn(id: &str) -> Turn {
    Turn {
        id: id.to_string(),
        items: Vec::new(),
        items_view: TurnItemsView::Full,
        status: TurnStatus::Completed,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}
