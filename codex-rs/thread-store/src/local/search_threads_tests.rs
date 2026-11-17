use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_rollout::ThreadItem;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use uuid::Uuid;

use super::super::LocalThreadStore;
use super::super::test_support::test_config;
use super::super::test_support::write_session_file_with;
use super::super::test_support::write_session_file_with_fork;
use super::ThreadSearchItem;
use super::cursor_from_thread_search_item;
use crate::ListThreadsParams;
use crate::SearchThreadsParams;
use crate::SortDirection;
use crate::ThreadRelationFilter;
use crate::ThreadSearchSourceKind;
use crate::ThreadSortKey;
use crate::ThreadStore;

#[test]
fn time_sort_cursors_include_thread_id_tie_breaker() {
    let thread_id = ThreadId::from_string("00000000-0000-0000-0000-000000000123")
        .expect("thread ID should parse");
    let item = ThreadSearchItem {
        item: ThreadItem {
            thread_id: Some(thread_id),
            created_at: Some("2026-01-27T12:34:56Z".to_string()),
            updated_at: Some("2026-01-27T12:34:56Z".to_string()),
            recency_at: Some("2026-01-27T12:34:56Z".to_string()),
            ..Default::default()
        },
        snippet: String::new(),
    };

    for sort_key in [
        ThreadSortKey::CreatedAt,
        ThreadSortKey::UpdatedAt,
        ThreadSortKey::RecencyAt,
    ] {
        let cursor = cursor_from_thread_search_item(&item, sort_key).expect("cursor should build");
        assert_eq!(
            serde_json::to_string(&cursor).expect("cursor should serialize"),
            format!("\"2026-01-27T12:34:56Z|{thread_id}\"")
        );
    }
}

#[tokio::test]
async fn authoritative_search_rechecks_stale_compressed_discovery() {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());
    let store = LocalThreadStore::new(config.clone(), /*state_db*/ None);
    let id = Uuid::new_v4();
    let path = write_session_file_with(
        home.path(),
        home.path().join("sessions/2025/01/03"),
        "2025-01-03T12-00-00",
        id,
        "changed message",
        Some("test-provider"),
        ThreadHistoryMode::Legacy,
    )
    .expect("write selected rollout");
    let contents = std::fs::read(&path).expect("read rollout");
    let compressed =
        zstd::stream::encode_all(contents.as_slice(), /*level*/ 3).expect("compress rollout");
    std::fs::write(path.with_extension("jsonl.zst"), compressed).expect("write compressed rollout");
    std::fs::remove_file(&path).expect("remove plain rollout");
    let rollout_config = codex_rollout::RolloutConfig {
        codex_home: config.codex_home.clone(),
        sqlite: config.sqlite.clone(),
        cwd: config.codex_home,
        model_provider_id: config.default_model_provider_id,
        generate_memories: false,
    };
    let params = SearchThreadsParams {
        include_attached_threads: false,
        account: None,
        page_size: 10,
        cursor: None,
        sort_key: ThreadSortKey::CreatedAt,
        sort_direction: SortDirection::Desc,
        allowed_sources: vec![SessionSource::Cli],
        source_kinds: None,
        archived: false,
        search_term: "needle".to_string(),
    };
    let mut matcher = codex_rollout::RolloutSnippetMatcher::new("needle");
    let page = super::finish_search(
        super::SearchContext {
            store: &store,
            params: &params,
            state_db: None,
            rollout_config: &rollout_config,
            cursor: None,
            sort_key: codex_rollout::ThreadSortKey::CreatedAt,
            sort_direction: codex_rollout::SortDirection::Desc,
            matcher: &mut matcher,
        },
        std::collections::HashSet::from([path]),
    )
    .await
    .expect("finish search");
    assert_eq!((page.items.len(), page.next_cursor), (0, None));
}

#[tokio::test]
async fn search_matches_selected_rollout_across_path_spellings_and_compression() {
    for compressed in [false, true] {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let state_db = codex_state::StateRuntime::init(
            config.sqlite.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("initialize state database");
        let uuid = Uuid::new_v4();
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("thread ID should parse");
        let selected_path = write_session_file_with(
            home.path(),
            home.path().join("sessions/2025/01/03"),
            "2025-01-03T12-00-00",
            uuid,
            "needle in selected rollout",
            Some("test-provider"),
            ThreadHistoryMode::Legacy,
        )
        .expect("write selected rollout");
        // Reverting a thread leaves its previous rollout on disk with the same thread ID.
        write_session_file_with(
            home.path(),
            home.path().join("sessions/2025/01/03"),
            "2025-01-03T11-00-00",
            uuid,
            "obsolete needle in previous rollout",
            Some("test-provider"),
            ThreadHistoryMode::Legacy,
        )
        .expect("write unselected rollout");

        let home_paths = [home.path().to_path_buf()];
        #[cfg(windows)]
        let home_paths = {
            let verbatim = std::fs::canonicalize(&home_paths[0]).expect("canonicalize home");
            let ordinary =
                codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(&verbatim)
                    .expect("normalize home")
                    .into_path_buf();
            [ordinary, verbatim]
        };
        let relative_path = selected_path
            .strip_prefix(home.path())
            .expect("relative rollout path");
        let selected_paths = home_paths.each_ref().map(|home| home.join(relative_path));
        #[cfg(windows)]
        assert_ne!(selected_paths[0], selected_paths[1]);

        if compressed {
            let contents = std::fs::read(&selected_path).expect("read selected rollout");
            let contents = zstd::stream::encode_all(contents.as_slice(), /*level*/ 3)
                .expect("compress selected rollout");
            std::fs::write(selected_path.with_extension("jsonl.zst"), contents)
                .expect("write compressed rollout");
            std::fs::remove_file(&selected_path).expect("remove plain rollout");
        }
        let selected_paths = selected_paths
            .into_iter()
            .flat_map(|path| {
                let compressed_path = compressed.then(|| path.with_extension("jsonl.zst"));
                std::iter::once(path).chain(compressed_path)
            })
            .collect::<Vec<_>>();

        for codex_home in home_paths {
            let store = LocalThreadStore::new(
                super::super::LocalThreadStoreConfig {
                    codex_home,
                    ..config.clone()
                },
                Some(state_db.clone()),
            );
            for selected_path in &selected_paths {
                let mut metadata = codex_state::ThreadMetadataBuilder::new(
                    thread_id,
                    selected_path.clone(),
                    Utc::now(),
                    SessionSource::Cli,
                )
                .build("test-provider");
                metadata.first_user_message = Some("needle in selected rollout".to_string());
                state_db
                    .upsert_thread(&metadata)
                    .await
                    .expect("select rollout in database");

                for (search_term, expected) in [
                    ("needle", vec![(thread_id, "needle in selected rollout")]),
                    ("obsolete", Vec::new()),
                ] {
                    let page = store
                        .search_threads(SearchThreadsParams {
                            include_attached_threads: false,
                            account: None,
                            page_size: 10,
                            cursor: None,
                            sort_key: ThreadSortKey::CreatedAt,
                            sort_direction: SortDirection::Desc,
                            allowed_sources: vec![SessionSource::Cli],
                            source_kinds: None,
                            archived: false,
                            search_term: search_term.to_string(),
                        })
                        .await
                        .expect("search selected rollout");
                    assert_eq!(
                        (
                            page.items
                                .iter()
                                .map(|item| (item.thread.thread_id, item.snippet.as_str()))
                                .collect::<Vec<_>>(),
                            page.next_cursor,
                        ),
                        (expected, None),
                        "compressed={compressed}, selected_path={selected_path:?}",
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn source_kind_filter_applies_before_search_pagination() {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());
    let state_db = codex_state::StateRuntime::init(
        config.sqlite.clone(),
        config.default_model_provider_id.clone(),
    )
    .await
    .expect("initialize state database");
    let mut review_ids = Vec::new();
    for (age, source) in [
        (0, SessionSource::SubAgent(SubAgentSource::Review)),
        (1, SessionSource::Cli),
        (2, SessionSource::SubAgent(SubAgentSource::Review)),
    ] {
        let uuid = Uuid::new_v4();
        let id = ThreadId::from_u128(uuid.as_u128());
        let path = write_session_file_with(
            home.path(),
            home.path().join("sessions/2025/01/03"),
            "2025-01-03T12-00-00",
            uuid,
            "needle message",
            Some("test-provider"),
            ThreadHistoryMode::Legacy,
        )
        .expect("write rollout");
        let mut metadata = codex_state::ThreadMetadataBuilder::new(
            id,
            path,
            Utc::now() - chrono::TimeDelta::days(age),
            source.clone(),
        )
        .build("test-provider");
        metadata.first_user_message = Some("needle message".to_string());
        state_db
            .upsert_thread(&metadata)
            .await
            .expect("index rollout");
        if matches!(source, SessionSource::SubAgent(SubAgentSource::Review)) {
            review_ids.push(id);
        }
    }

    let store = LocalThreadStore::new(config, Some(state_db));
    let mut params = SearchThreadsParams {
        include_attached_threads: false,
        account: None,
        page_size: 1,
        cursor: None,
        sort_key: ThreadSortKey::CreatedAt,
        sort_direction: SortDirection::Desc,
        allowed_sources: Vec::new(),
        source_kinds: Some(vec![ThreadSearchSourceKind::SubAgentReview]),
        archived: false,
        search_term: "needle".to_string(),
    };
    let first = store
        .search_threads(params.clone())
        .await
        .expect("first page");
    params.cursor = first.next_cursor.clone();
    let second = store.search_threads(params).await.expect("second page");
    assert_eq!(
        (
            first
                .items
                .into_iter()
                .map(|item| item.thread.thread_id)
                .collect::<Vec<_>>(),
            first.next_cursor.is_some(),
            second
                .items
                .into_iter()
                .map(|item| item.thread.thread_id)
                .collect::<Vec<_>>(),
            second.next_cursor,
        ),
        (vec![review_ids[0]], true, vec![review_ids[1]], None),
    );
}

#[tokio::test]
async fn searches_are_independent_across_catalog_refills() -> Result<(), Box<dyn std::error::Error>>
{
    let home = TempDir::new()?;
    let config = test_config(home.path());
    let db = codex_state::StateRuntime::init(
        config.sqlite.clone(),
        config.default_model_provider_id.clone(),
    )
    .await?;
    let store = LocalThreadStore::new(config, Some(db.clone()));
    // At this page size the catalog returns 256 rows. Put a hit on either side.
    let created_at = Utc::now();
    for id in 1..=258 {
        let message = match id {
            258 => "first needle",
            257 => "other query",
            1 => "last NEEDLE",
            _ => "ordinary conversation",
        };
        let path = write_session_file_with(
            home.path(),
            home.path().join("sessions/2025/01/03"),
            "2025-01-03T12-00-00",
            Uuid::from_u128(id),
            message,
            Some("test-provider"),
            ThreadHistoryMode::Legacy,
        )?;
        let mut row = codex_state::ThreadMetadataBuilder::new(
            ThreadId::from_u128(id),
            path,
            created_at,
            SessionSource::Cli,
        )
        .build("test-provider");
        row.first_user_message = Some(message.to_owned());
        db.upsert_thread(&row).await?;
    }
    let search = async |query: &str| {
        let page = store
            .search_threads(SearchThreadsParams {
                include_attached_threads: false,
                account: None,
                page_size: 2,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: vec![SessionSource::Cli],
                source_kinds: None,
                archived: false,
                search_term: query.to_owned(),
            })
            .await?;
        Ok::<_, crate::ThreadStoreError>((
            page.items
                .into_iter()
                .map(|item| (item.thread.thread_id, item.snippet))
                .collect::<Vec<_>>(),
            page.next_cursor,
        ))
    };
    let (many, one, none) = tokio::try_join!(search("needle"), search("other"), search("absent"))?;
    assert_eq!(
        (many, one, none),
        (
            (
                vec![
                    (
                        ThreadId::from_u128(/*value*/ 258),
                        "first needle".to_owned()
                    ),
                    (ThreadId::from_u128(/*value*/ 1), "last NEEDLE".to_owned()),
                ],
                None,
            ),
            (
                vec![(ThreadId::from_u128(/*value*/ 257), "other query".to_owned())],
                None
            ),
            (vec![], None),
        )
    );
    Ok(())
}

async fn write_named_rollout(home: &Path, id: Uuid, attached_to: Option<Uuid>) -> PathBuf {
    let path = write_session_file_with_fork(
        home,
        home.join("sessions/2025/01/03"),
        "2025-01-03T12-00-00",
        id,
        "searchable message",
        Some("test-provider"),
        attached_to,
        attached_to.is_some(),
        ThreadHistoryMode::Legacy,
    )
    .expect("write rollout");
    codex_rollout::append_thread_name(home, ThreadId::from_u128(id.as_u128()), "matching title")
        .await
        .expect("save thread name");
    path
}

fn inclusive_title_search() -> ListThreadsParams {
    ListThreadsParams {
        included_thread_ids: None,
        page_size: 10,
        cursor: None,
        sort_key: ThreadSortKey::CreatedAt,
        sort_direction: SortDirection::Desc,
        allowed_sources: vec![SessionSource::Cli],
        model_providers: None,
        cwd_filters: None,
        section: None,
        project_id: None,
        archived: false,
        search_term: Some("matching title".to_string()),
        relation_filter: Some(ThreadRelationFilter::IncludeAttached),
        use_state_db_only: false,
    }
}

#[tokio::test]
async fn title_and_content_search_return_attached_parent_without_state_db() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let parent = Uuid::new_v4();
    let child = Uuid::new_v4();
    write_named_rollout(home.path(), child, Some(parent)).await;
    let expected = vec![(
        ThreadId::from_u128(child.as_u128()),
        Some(ThreadId::from_u128(parent.as_u128())),
    )];

    let list = store
        .list_threads(inclusive_title_search())
        .await
        .expect("search titles on disk");
    assert_eq!(
        list.items
            .into_iter()
            .map(|t| (t.thread_id, t.attached_to_thread_id))
            .collect::<Vec<_>>(),
        expected
    );

    let search = store
        .search_threads(SearchThreadsParams {
            page_size: 10,
            cursor: None,
            sort_key: ThreadSortKey::CreatedAt,
            sort_direction: SortDirection::Desc,
            allowed_sources: vec![SessionSource::Cli],
            source_kinds: None,
            archived: false,
            search_term: "searchable message".to_string(),
            include_attached_threads: true,
            account: None,
        })
        .await
        .expect("search messages on disk");
    assert_eq!(
        search
            .items
            .into_iter()
            .map(|hit| (hit.thread.thread_id, hit.thread.attached_to_thread_id))
            .collect::<Vec<_>>(),
        expected
    );
}

#[tokio::test]
async fn inclusive_title_search_finds_unindexed_matches_when_filesystem_scanning_is_allowed() {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());
    let state_db = codex_state::StateRuntime::init(
        config.sqlite.clone(),
        config.default_model_provider_id.clone(),
    )
    .await
    .expect("initialize state database");
    let parent = Uuid::new_v4();
    let child = Uuid::new_v4();
    let standalone = Uuid::new_v4();
    let parent_path = write_named_rollout(home.path(), parent, /*attached_to*/ None).await;
    write_named_rollout(home.path(), standalone, /*attached_to*/ None).await;
    write_named_rollout(home.path(), child, Some(parent)).await;
    let parent_id = ThreadId::from_u128(parent.as_u128());
    let child_id = ThreadId::from_u128(child.as_u128());
    let mut row = codex_state::ThreadMetadataBuilder::new(
        parent_id,
        parent_path,
        Utc::now(),
        SessionSource::Cli,
    )
    .build("test-provider");
    row.first_user_message = Some("searchable message".to_string());
    row.title = "matching title".to_string();
    state_db
        .upsert_thread(&row)
        .await
        .expect("index only the parent");
    let store = LocalThreadStore::new(config, Some(state_db));
    let all = HashMap::from([
        (parent_id, None),
        (child_id, Some(parent_id)),
        (ThreadId::from_u128(standalone.as_u128()), None),
    ]);
    for (use_state_db_only, expected) in [
        (true, HashMap::from([(parent_id, None)])),
        (false, all.clone()),
        (true, all),
    ] {
        let page = store
            .list_threads(ListThreadsParams {
                use_state_db_only,
                ..inclusive_title_search()
            })
            .await
            .expect("search titles");
        assert_eq!(
            page.items
                .into_iter()
                .map(|t| (t.thread_id, t.attached_to_thread_id))
                .collect::<HashMap<_, _>>(),
            expected
        );
    }
}

#[tokio::test]
async fn streaming_search_never_reports_undiscovered_rows_from_a_shared_database() {
    let home = TempDir::new().unwrap();
    let other = TempDir::new().unwrap();
    let config = test_config(home.path());
    let db = codex_state::StateRuntime::init(
        config.sqlite.clone(),
        config.default_model_provider_id.clone(),
    )
    .await
    .unwrap();
    let uuid = Uuid::new_v4();
    let path = write_session_file_with(
        other.path(),
        other.path().join("sessions/2025/01/03"),
        "2025-01-03T12-00-00",
        uuid,
        "private needle",
        Some("test-provider"),
        ThreadHistoryMode::Legacy,
    )
    .unwrap();
    let metadata = codex_state::ThreadMetadataBuilder::new(
        ThreadId::from_u128(uuid.as_u128()),
        path,
        Utc::now(),
        SessionSource::Cli,
    )
    .build("test-provider");
    db.upsert_thread(&metadata).await.unwrap();
    let store = LocalThreadStore::new(config, Some(db));
    let params = SearchThreadsParams {
        include_attached_threads: false,
        account: None,
        page_size: 10,
        cursor: None,
        sort_key: ThreadSortKey::CreatedAt,
        sort_direction: SortDirection::Desc,
        allowed_sources: vec![SessionSource::Cli],
        source_kinds: None,
        archived: false,
        search_term: "needle".into(),
    };
    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let page = store
        .search_threads_observed(
            params,
            crate::ThreadSearchObserver {
                progress: Some(tx),
                cancelled: Box::pin(std::future::pending()),
            },
        )
        .await
        .unwrap();
    assert!(page.items.is_empty());
    assert!(rx.recv().await.is_none());
}
