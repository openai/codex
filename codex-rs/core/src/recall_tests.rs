use std::fs::File;
use std::io::Write;

use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use uuid::Uuid;

use crate::config::test_config;

use super::*;

#[tokio::test]
async fn search_sessions_ranks_matching_rollout_first() {
    let codex_home = tempdir().expect("tempdir");
    let mut config = test_config();
    config.codex_home = codex_home.path().to_path_buf();
    config.model_provider_id = "openai".to_string();

    let hit_id = Uuid::new_v4();
    let miss_id = Uuid::new_v4();
    write_rollout(
        codex_home.path(),
        hit_id,
        "2026-03-19T10-00-00",
        "we debugged bm25 recall over old sessions",
        "The implementation used a sparse text index.",
    );
    write_rollout(
        codex_home.path(),
        miss_id,
        "2026-03-19T09-00-00",
        "we reviewed terminal colors",
        "No search work happened in this session.",
    );

    let hits = search_sessions(&config, "bm25 recall", /*current_thread_id*/ None, 2)
        .await
        .expect("search sessions");

    assert!(!hits.is_empty(), "expected at least one recall hit");
    assert_eq!(hits[0].thread_id.to_string(), hit_id.to_string());
    assert_eq!(
        hits[0].snippet,
        "we debugged bm25 recall over old sessions".to_string()
    );
}

#[test]
fn format_recall_draft_includes_source_metadata_and_excerpt() {
    let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
    let hits = vec![SessionRecallHit {
        thread_id,
        score: 1.5,
        thread_name: Some("Recall prototype".to_string()),
        path: "/tmp/rollout.jsonl".into(),
        created_at: Some("2026-03-19T10:00:00Z".to_string()),
        updated_at: Some("2026-03-19T10:05:00Z".to_string()),
        cwd: Some("/Users/starr/code/codex".into()),
        snippet: "BM25 matched the old session.".to_string(),
    }];

    let draft = format_recall_draft("bm25", &hits);

    assert_eq!(
        draft,
        format!(
            "Recalled context from previous Codex sessions for query: bm25\n\n\
             1. Recall prototype\n\
             Thread: {thread_id}\n\
             Updated: 2026-03-19T10:05:00Z\n\
             Cwd: /Users/starr/code/codex\n\
             Rollout: /tmp/rollout.jsonl\n\
             Excerpt: BM25 matched the old session.\n\n\
             Use this recalled context if it is relevant to my next message."
        )
    );
}

fn write_rollout(
    codex_home: &std::path::Path,
    uuid: Uuid,
    ts: &str,
    user_message: &str,
    assistant_message: &str,
) {
    let day_dir = codex_home.join("sessions/2026/03/19");
    std::fs::create_dir_all(&day_dir).expect("create day dir");
    let path = day_dir.join(format!("rollout-{ts}-{uuid}.jsonl"));
    let mut file = File::create(&path).expect("create rollout");
    let thread_id = ThreadId::from_string(&uuid.to_string()).expect("thread id");

    let session_meta = RolloutLine {
        timestamp: format!("{ts}Z"),
        item: RolloutItem::SessionMeta(SessionMetaLine {
            meta: SessionMeta {
                id: thread_id,
                forked_from_id: None,
                timestamp: format!("{ts}Z"),
                cwd: codex_home.to_path_buf(),
                originator: "cli".to_string(),
                cli_version: "test".to_string(),
                source: SessionSource::Cli,
                agent_nickname: None,
                agent_role: None,
                model_provider: Some("openai".to_string()),
                base_instructions: None,
                dynamic_tools: None,
                memory_mode: None,
            },
            git: None,
        }),
    };
    writeln!(
        file,
        "{}",
        serde_json::to_string(&session_meta).expect("serialize session meta")
    )
    .expect("write session meta");

    let user = RolloutLine {
        timestamp: format!("{ts}Z"),
        item: RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: user_message.to_string(),
            images: None,
            text_elements: Vec::new(),
            local_images: Vec::new(),
        })),
    };
    writeln!(
        file,
        "{}",
        serde_json::to_string(&user).expect("serialize user message")
    )
    .expect("write user message");

    let assistant = RolloutLine {
        timestamp: format!("{ts}Z"),
        item: RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: assistant_message.to_string(),
            }],
            end_turn: None,
            phase: None,
        }),
    };
    writeln!(
        file,
        "{}",
        serde_json::to_string(&assistant).expect("serialize assistant message")
    )
    .expect("write assistant message");
}
