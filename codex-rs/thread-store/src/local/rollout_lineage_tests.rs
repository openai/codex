use std::fs;
use std::io::Write;
use std::path::Path;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutLine;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::super::LocalThreadStore;
use super::super::test_support::test_config;
use super::RolloutLineageSegment;
use crate::ForkBoundary;
use crate::LoadThreadHistoryParams;
use crate::PrepareForkParams;
use crate::ThreadStore;

#[cfg(unix)]
#[tokio::test]
async fn rejects_reference_lineage_escaping_symlinked_sessions_root() {
    let home = TempDir::new().expect("temp dir");
    let external = TempDir::new().expect("external temp dir");
    let external_sessions = external.path().join("sessions");
    fs::create_dir_all(external_sessions.as_path()).expect("create external sessions");
    std::os::unix::fs::symlink(external_sessions.as_path(), home.path().join("sessions"))
        .expect("symlink sessions");
    let escaped = TempDir::new().expect("escaped temp dir");
    let escaped_rollouts = escaped.path().join("rollouts");
    fs::create_dir_all(escaped_rollouts.as_path()).expect("create escaped rollouts");
    std::os::unix::fs::symlink(escaped_rollouts.as_path(), external_sessions.join("2026"))
        .expect("symlink nested sessions directory");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::default();
    write_rollout(
        home.path(),
        thread_id,
        /*history_base*/ None,
        /*next_ordinal*/ 3,
    );

    let error = store
        .resolve_rollout_lineage_for_reference(thread_id)
        .await
        .expect_err("escaping reference lineage should be rejected");

    assert!(error.to_string().contains("must be in Codex home"));
}

#[tokio::test]
async fn resolves_nested_lineage_with_empty_intermediate_segments() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let root = ThreadId::default();
    let middle = ThreadId::default();
    let child = ThreadId::default();
    let root_path = write_rollout(
        home.path(),
        root,
        /*history_base*/ None,
        /*next_ordinal*/ 6,
    );
    let root_end = history_position(root_path.as_path(), root, /*end_ordinal_exclusive*/ 4);
    let middle_path = write_rollout(home.path(), middle, Some(root_end), /*next_ordinal*/ 1);
    let middle_end = history_position(
        middle_path.as_path(),
        middle,
        /*end_ordinal_exclusive*/ 5,
    );
    let child_path = write_rollout(
        home.path(),
        child,
        Some(middle_end),
        /*next_ordinal*/ 3,
    );

    let lineage = store
        .resolve_rollout_lineage(child, /*initial_path*/ None)
        .await
        .expect("resolve nested lineage");

    assert_eq!(
        lineage.segments,
        vec![
            RolloutLineageSegment {
                rollout_id: root,
                rollout_path: root_path.clone(),
                start_ordinal: 1,
                end: Some(root_end),
                hides_parent: false,
            },
            RolloutLineageSegment {
                rollout_id: middle,
                rollout_path: middle_path.clone(),
                start_ordinal: 5,
                end: Some(middle_end),
                hides_parent: false,
            },
            RolloutLineageSegment {
                rollout_id: child,
                rollout_path: child_path,
                start_ordinal: 6,
                end: None,
                hides_parent: false,
            },
        ]
    );
}

#[tokio::test]
async fn resolves_archived_ancestors() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let root = ThreadId::default();
    let child = ThreadId::default();
    let root_path = write_rollout_under(
        home.path().join("archived_sessions"),
        root,
        /*history_base*/ None,
        /*next_ordinal*/ 3,
    );
    write_rollout(
        home.path(),
        child,
        Some(history_position(
            root_path.as_path(),
            root,
            /*end_ordinal_exclusive*/ 3,
        )),
        /*next_ordinal*/ 2,
    );

    let lineage = store
        .resolve_rollout_lineage(child, /*initial_path*/ None)
        .await
        .expect("resolve archived ancestor");

    assert_eq!(lineage.segments[0].rollout_path, root_path);
}

#[tokio::test]
async fn unreadable_ancestor_recovers_from_another_copy_of_the_same_rollout() {
    let temp = TempDir::new().expect("temp dir");
    let home = temp.path().canonicalize().expect("canonical home");
    let store = store_with_state(&home).await;
    let owner = ThreadId::new();
    let root = ThreadId::new();
    let child = ThreadId::new();
    let original = write_rollout(
        &home, owner, /*history_base*/ None, /*next_ordinal*/ 5,
    );
    // Reverted rollout filenames contain the owner's stable thread ID and a distinct rollout ID.
    let name = format!("rollout-2026-07-16T00-00-00-{owner}_{root}.jsonl");
    let active = original.with_file_name(&name);
    fs::rename(original, &active).expect("name reverted rollout");
    let end = history_position(&active, root, /*end_ordinal_exclusive*/ 3);
    let child_path = write_rollout(&home, child, Some(end), /*next_ordinal*/ 2);
    let archived_dir = home.join("archived_sessions");
    fs::create_dir_all(&archived_dir).expect("create archive");
    let archived = archived_dir.join(name);
    let contents = fs::read_to_string(&active).expect("read active rollout");
    let load_context = || {
        store.load_latest_model_context(LoadThreadHistoryParams {
            thread_id: child,
            include_archived: false,
        })
    };
    let expected = load_context().await.expect("load intact context").items;
    let expected = serde_json::to_value(expected).expect("serialize context");
    let compressed = active.with_extension("jsonl.zst");
    let header = contents
        .split_inclusive('\n')
        .next()
        .expect("session header");
    write_unfinished_frame(&compressed, header);
    fs::remove_file(&active).expect("remove plain rollout");

    let assert_original_error = async || {
        let err = load_context().await.expect_err("no usable copy");
        assert!(
            err.to_string()
                .ends_with(&format!("{}: incomplete frame", compressed.display())),
            "{err}"
        );
    };
    assert_original_error().await;
    // History pages read projected rows and only need the ancestor header for its bounds.
    let page = store
        .resolve_rollout_lineage_for_history_page(child)
        .await
        .expect("page with only the short active copy");
    assert_eq!(page.segments[0].rollout_path, compressed);
    let child_contents = fs::read_to_string(&child_path).expect("read child");
    let checkpoint = serde_json::json!({
        "timestamp": "2026-07-16T00:00:00.000Z", "ordinal": 5, "type": "compacted",
        "payload": {"message": "child checkpoint", "replacement_history": [], "window_number": 1}
    });
    fs::write(&child_path, format!("{child_contents}{checkpoint}\n")).expect("append checkpoint");
    let context = load_context()
        .await
        .expect("resume at the newer checkpoint");
    assert!(matches!(
        context.items.as_slice(),
        [RolloutItem::SessionMeta(_), RolloutItem::Compacted(item)] if item.message == "child checkpoint"
    ));
    fs::write(&child_path, child_contents).expect("restore child");
    fs::write(
        &archived,
        contents.replace(&owner.to_string(), &root.to_string()),
    )
    .expect("stage another owner's history");
    assert_original_error().await;

    let archive = contents.split_inclusive('\n').take(4).collect::<String>();
    fs::write(&archived, archive).expect("restore matching archived copy");
    let context = load_context().await.expect("resume from archive").items;
    assert_eq!(
        serde_json::to_value(context).expect("serialize resumed context"),
        expected
    );
    let fork = store
        .prepare_fork(PrepareForkParams {
            thread_id: child,
            boundary: ForkBoundary::Latest,
        })
        .await
        .expect("fork from archive");
    assert_eq!(
        serde_json::to_value(fork.model_context.as_ref()).expect("serialize fork context"),
        expected
    );

    let assert_archive_projected = async |expected_offset| {
        let err = store
            .prepare_fork(PrepareForkParams {
                thread_id: child,
                boundary: ForkBoundary::ThroughTurn("missing".to_string()),
            })
            .await
            .expect_err("turn is missing after projection");
        assert!(
            matches!(&err, crate::ThreadStoreError::InvalidRequest { message } if message == "turn not found: missing"),
            "{err}"
        );
        let projection = super::super::thread_history::projection_state(&store, root)
            .await
            .expect("read checkpoint")
            .expect("ancestor projected");
        assert_eq!(projection.next_byte_offset, expected_offset);
    };
    assert_archive_projected(end.end_byte_offset).await;

    // The ancestor's own projection can advance beyond the child's inherited prefix.
    super::super::thread_history_materialization::materialize_to_sqlite(&store, root, &archived)
        .await
        .expect("project the rest of the archive");
    let projected = fs::metadata(&archived).expect("archive size").len();
    let prefix = &contents.as_bytes()[..end.end_byte_offset as usize];
    let compressed_prefix = zstd::encode_all(prefix, /*level*/ 0).expect("compress prefix");
    fs::write(&compressed, compressed_prefix).expect("replace active copy with a shorter stream");
    fs::write(&archived, prefix).expect("shorten archive to the inherited prefix");
    assert_archive_projected(projected).await;
}

async fn store_with_state(home: &Path) -> LocalThreadStore {
    let config = test_config(home);
    let state = codex_state::StateRuntime::init(
        config.sqlite.clone(),
        config.default_model_provider_id.clone(),
    )
    .await
    .expect("initialize sqlite");
    LocalThreadStore::new(config, Some(state))
}

fn write_unfinished_frame(path: &Path, prefix: &str) {
    let mut encoder = zstd::Encoder::new(Vec::new(), /*level*/ 0).expect("create encoder");
    encoder
        .set_pledged_src_size(Some(prefix.len() as u64))
        .expect("set frame size");
    encoder.write_all(prefix.as_bytes()).expect("write prefix");
    encoder.flush().expect("flush unfinished frame");
    fs::write(path, encoder.get_ref()).expect("write unfinished frame");
}

#[tokio::test]
async fn indexed_ancestor_must_be_managed_and_owned_by_the_indexed_thread() {
    let temp = TempDir::new().expect("temp dir");
    let home = temp.path().canonicalize().expect("canonical home");
    let external_home = TempDir::new().expect("external home");
    let store = store_with_state(&home).await;
    let state = store.state_db().await.expect("SQLite store");
    let root = ThreadId::new();
    let child = ThreadId::new();
    let active = write_rollout(
        &home, root, /*history_base*/ None, /*next_ordinal*/ 3,
    );
    let archived = write_rollout_under(
        home.join("archived_sessions"),
        root,
        /*history_base*/ None,
        /*next_ordinal*/ 4,
    );
    let external = write_rollout(
        external_home.path(),
        root,
        /*history_base*/ None,
        /*next_ordinal*/ 3,
    );
    let end = history_position(&active, root, /*end_ordinal_exclusive*/ 3);
    write_rollout(&home, child, Some(end), /*next_ordinal*/ 2);
    let assert_lookup = async |indexed: &Path, expected: &Path| {
        let mut metadata = codex_state::ThreadMetadataBuilder::new(
            root,
            indexed.to_owned(),
            chrono::Utc::now(),
            codex_protocol::protocol::SessionSource::Cli,
        );
        metadata.history_mode = ThreadHistoryMode::Paginated;
        state
            .upsert_thread(&metadata.build("test-provider"))
            .await
            .expect("index ancestor");
        let existing = store
            .resolve_rollout_lineage(child, /*initial_path*/ None)
            .await
            .expect("resolve ancestor");
        let reference = store
            .resolve_rollout_lineage_for_reference(child)
            .await
            .expect("resolve reference");
        for lineage in [existing, reference] {
            assert_eq!(lineage.segments[0].rollout_path, expected);
        }
    };
    assert_lookup(&archived, &archived).await;
    assert_lookup(&external, &active).await;
    let contents = fs::read_to_string(&archived).expect("read archive");
    let (header, events) = contents.split_once('\n').expect("header and events");
    let legacy = header.replace("\"paginated\"", "\"legacy\"");
    fs::write(&archived, header).expect("shorten indexed archive");
    assert_lookup(&archived, &archived).await;
    fs::write(&archived, format!("{legacy}\n{events}")).expect("write legacy archive");
    assert_lookup(&archived, &active).await;
    let compressed = archived.with_extension("jsonl.zst");
    fs::write(
        &compressed,
        zstd::encode_all(contents.as_bytes(), /*level*/ 0).expect("compress archive"),
    )
    .expect("write complete compressed copy");
    fs::remove_file(&archived).expect("remove plain archive");
    assert_lookup(&compressed, &compressed).await;
    let load_context = || {
        store.load_latest_model_context(LoadThreadHistoryParams {
            thread_id: child,
            include_archived: false,
        })
    };
    let items = async || {
        serde_json::to_value(load_context().await.expect("load context").items)
            .expect("serialize context")
    };
    let expected = items().await;
    for prefix in [contents.as_str(), &contents[..header.len() + 1]] {
        write_unfinished_frame(&compressed, prefix);
        assert_lookup(&compressed, &compressed).await;
        assert_eq!(items().await, expected);
        let fork = store
            .prepare_fork(PrepareForkParams {
                thread_id: child,
                boundary: ForkBoundary::Latest,
            })
            .await
            .expect("fork using the intact active copy");
        assert_eq!(
            serde_json::to_value(fork.model_context.as_ref()).expect("serialize fork"),
            expected
        );
        let page = store
            .resolve_rollout_lineage_for_history_page(child)
            .await
            .expect("resolve page");
        assert_eq!(page.segments[0].rollout_path, compressed);
    }
    let wrong_owner = contents.replace(&root.to_string(), &ThreadId::new().to_string());
    fs::write(&active, &wrong_owner).expect("substitute another thread's history");
    let error = load_context()
        .await
        .expect_err("reject wrong owner on retry");
    assert!(
        error
            .to_string()
            .ends_with(&format!("{}: incomplete frame", compressed.display())),
        "{error}"
    );
    fs::remove_file(&compressed).expect("remove truncated archive");
    fs::write(&archived, &contents).expect("restore archive");
    assert_lookup(&active, &archived).await;
    fs::write(&active, []).expect("empty indexed rollout");
    assert_lookup(&active, &archived).await;
    fs::remove_file(&archived).expect("remove fallback");
    assert_invalid_lineage(
        &store,
        child,
        &format!("rollout at {} is empty", active.display()),
    )
    .await;

    #[cfg(unix)]
    {
        fs::copy(&external, &active).expect("restore active rollout");
        fs::write(
            &compressed,
            zstd::encode_all(contents.as_bytes(), /*level*/ 0).expect("compress archive"),
        )
        .expect("restore compressed archive");
        std::os::unix::fs::symlink(&external, &archived).expect("link plain sibling outside home");
        let alias = home
            .join("sessions")
            .join(compressed.file_name().expect("filename"));
        std::os::unix::fs::symlink(&compressed, &alias).expect("link indexed compressed rollout");
        assert_lookup(&alias, &active).await;
    }
}

#[tokio::test]
async fn resolves_lineage_at_explicit_history_position() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let root = ThreadId::default();
    let child = ThreadId::default();
    let root_path = write_rollout(
        home.path(),
        root,
        /*history_base*/ None,
        /*next_ordinal*/ 6,
    );
    let root_end = history_position(root_path.as_path(), root, /*end_ordinal_exclusive*/ 4);
    let child_path = write_rollout(home.path(), child, Some(root_end), /*next_ordinal*/ 4);
    let end = history_position(
        child_path.as_path(),
        child,
        /*end_ordinal_exclusive*/ 6,
    );

    let lineage = store
        .resolve_rollout_lineage(child, /*initial_path*/ None)
        .await
        .expect("resolve child lineage")
        .truncate_at(end)
        .await
        .expect("resolve explicit position");

    assert_eq!(
        lineage.segments,
        vec![
            RolloutLineageSegment {
                rollout_id: root,
                rollout_path: root_path.clone(),
                start_ordinal: 1,
                end: Some(root_end),
                hides_parent: false,
            },
            RolloutLineageSegment {
                rollout_id: child,
                rollout_path: child_path.clone(),
                start_ordinal: 5,
                end: Some(end),
                hides_parent: false,
            },
        ]
    );
}

#[tokio::test]
async fn rejects_missing_cycles_and_out_of_bounds_offsets() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let missing_parent = ThreadId::default();
    let missing_child = ThreadId::default();
    write_rollout(
        home.path(),
        missing_child,
        Some(unchecked_history_position(
            missing_parent,
            /*end_ordinal_exclusive*/ 1,
        )),
        /*next_ordinal*/ 2,
    );
    assert_invalid_lineage(&store, missing_child, "missing source rollout").await;

    let cycle_a = ThreadId::default();
    let cycle_b = ThreadId::default();
    write_rollout(
        home.path(),
        cycle_a,
        Some(unchecked_history_position(
            cycle_b, /*end_ordinal_exclusive*/ 1,
        )),
        /*next_ordinal*/ 2,
    );
    write_rollout(
        home.path(),
        cycle_b,
        Some(unchecked_history_position(
            cycle_a, /*end_ordinal_exclusive*/ 1,
        )),
        /*next_ordinal*/ 2,
    );
    assert_invalid_lineage(&store, cycle_a, "cycle detected").await;

    let root = ThreadId::default();
    let invalid_child = ThreadId::default();
    let root_path = write_rollout(
        home.path(),
        root,
        /*history_base*/ None,
        /*next_ordinal*/ 2,
    );
    write_rollout(
        home.path(),
        invalid_child,
        Some(HistoryPosition {
            thread_id: root,
            end_ordinal_exclusive: 2,
            end_byte_offset: fs::metadata(root_path).expect("root metadata").len() + 1,
        }),
        /*next_ordinal*/ 2,
    );
    assert_invalid_lineage(
        &store,
        invalid_child,
        "cutoff byte offset is past the source rollout",
    )
    .await;
}

async fn assert_invalid_lineage(store: &LocalThreadStore, thread_id: ThreadId, detail: &str) {
    let err = store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id,
            include_archived: false,
        })
        .await
        .expect_err("lineage should be invalid");
    assert!(err.to_string().contains(detail), "{err}");
}

fn write_rollout(
    home: &Path,
    thread_id: ThreadId,
    history_base: Option<HistoryPosition>,
    next_ordinal: u64,
) -> std::path::PathBuf {
    write_rollout_under(
        home.join("sessions/2026/07/16"),
        thread_id,
        history_base,
        next_ordinal,
    )
}

fn write_rollout_under(
    directory: std::path::PathBuf,
    thread_id: ThreadId,
    history_base: Option<HistoryPosition>,
    next_ordinal: u64,
) -> std::path::PathBuf {
    fs::create_dir_all(directory.as_path()).expect("create rollout directory");
    let path = directory.join(format!("rollout-2026-07-16T00-00-00-{thread_id}.jsonl"));
    let initial_ordinal = history_base.map_or(0, |base| base.end_ordinal_exclusive);
    let mut lines = vec![rollout_line(
        initial_ordinal,
        RolloutItem::SessionMeta(Box::new(SessionMetaLine {
            meta: SessionMeta {
                session_id: thread_id.into(),
                id: thread_id,
                history_mode: ThreadHistoryMode::Paginated,
                history_base,
                ..SessionMeta::default()
            },
            git: None,
        })),
    )];
    for offset in 1..next_ordinal {
        let ordinal = initial_ordinal
            .checked_add(offset)
            .expect("fixture ordinal");
        lines.push(rollout_line(
            ordinal,
            RolloutItem::EventMsg(codex_protocol::protocol::EventMsg::ShutdownComplete),
        ));
    }
    fs::write(path.as_path(), format!("{}\n", lines.join("\n"))).expect("write rollout");
    path
}

fn rollout_line(ordinal: u64, item: RolloutItem) -> String {
    serde_json::to_string(&RolloutLine {
        timestamp: "2026-07-16T00:00:00.000Z".to_string(),
        ordinal: Some(ordinal),
        item,
    })
    .expect("serialize rollout line")
}

fn history_position(
    path: &Path,
    thread_id: ThreadId,
    end_ordinal_exclusive: u64,
) -> HistoryPosition {
    HistoryPosition {
        thread_id,
        end_ordinal_exclusive,
        end_byte_offset: rollout_end_byte_offset(path, end_ordinal_exclusive),
    }
}

fn rollout_end_byte_offset(path: &Path, end_ordinal_exclusive: u64) -> u64 {
    let bytes = fs::read(path).expect("read rollout");
    let end_byte_offset = bytes
        .split_inclusive(|byte| *byte == b'\n')
        .take_while(|line| {
            codex_rollout::parse_rollout_line_bytes(line)
                .expect("parse rollout fixture")
                .ordinal
                .expect("paginated rollout ordinal")
                < end_ordinal_exclusive
        })
        .map(<[u8]>::len)
        .sum::<usize>();
    u64::try_from(end_byte_offset).expect("rollout byte offset fits u64")
}

fn unchecked_history_position(thread_id: ThreadId, end_ordinal_exclusive: u64) -> HistoryPosition {
    HistoryPosition {
        thread_id,
        end_ordinal_exclusive,
        end_byte_offset: 0,
    }
}
