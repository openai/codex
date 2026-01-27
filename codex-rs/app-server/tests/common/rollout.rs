use anyhow::Result;
use codex_protocol::ThreadId;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use serde_json::json;
use std::fs;
use std::fs::FileTimes;
use std::path::Path;
use std::path::PathBuf;
use uuid::Uuid;

pub fn rollout_path(codex_home: &Path, filename_ts: &str, thread_id: &str) -> PathBuf {
    let year = &filename_ts[0..4];
    let month = &filename_ts[5..7];
    let day = &filename_ts[8..10];
    codex_home
        .join("sessions")
        .join(year)
        .join(month)
        .join(day)
        .join(format!("rollout-{filename_ts}-{thread_id}.jsonl"))
}

/// Create a minimal rollout file under `CODEX_HOME/sessions/YYYY/MM/DD/`.
///
/// - `filename_ts` is the filename timestamp component in `YYYY-MM-DDThh-mm-ss` format.
/// - `meta_rfc3339` is the envelope timestamp used in JSON lines.
/// - `preview` is the user message preview text.
/// - `model_provider` optionally sets the provider in the session meta payload.
///
/// Returns the generated conversation/session UUID as a string.
pub fn create_fake_rollout(
    codex_home: &Path,
    filename_ts: &str,
    meta_rfc3339: &str,
    preview: &str,
    model_provider: Option<&str>,
    git_info: Option<GitInfo>,
) -> Result<String> {
    create_fake_rollout_with_source(
        codex_home,
        filename_ts,
        meta_rfc3339,
        preview,
        model_provider,
        git_info,
        SessionSource::Cli,
    )
}

/// Create a minimal rollout file with an explicit session source.
pub fn create_fake_rollout_with_source(
    codex_home: &Path,
    filename_ts: &str,
    meta_rfc3339: &str,
    preview: &str,
    model_provider: Option<&str>,
    git_info: Option<GitInfo>,
    source: SessionSource,
) -> Result<String> {
    let uuid = Uuid::new_v4();
    let uuid_str = uuid.to_string();
    let conversation_id = ThreadId::from_string(&uuid_str)?;

    let file_path = rollout_path(codex_home, filename_ts, &uuid_str);
    let dir = file_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("missing rollout parent directory"))?;
    fs::create_dir_all(dir)?;

    // Build JSONL lines
    let meta = SessionMeta {
        id: conversation_id,
        forked_from_id: None,
        timestamp: meta_rfc3339.to_string(),
        cwd: PathBuf::from("/"),
        originator: "codex".to_string(),
        cli_version: "0.0.0".to_string(),
        source,
        model_provider: model_provider.map(str::to_string),
        base_instructions: None,
    };
    let payload = serde_json::to_value(SessionMetaLine {
        meta,
        git: git_info,
    })?;

    let lines = [
        json!({
            "timestamp": meta_rfc3339,
            "type": "session_meta",
            "payload": payload
        })
        .to_string(),
        json!({
            "timestamp": meta_rfc3339,
            "type":"response_item",
            "payload": {
                "type":"message",
                "role":"user",
                "content":[{"type":"input_text","text": preview}]
            }
        })
        .to_string(),
        json!({
            "timestamp": meta_rfc3339,
            "type":"event_msg",
            "payload": {
                "type":"user_message",
                "message": preview,
                "kind": "plain"
            }
        })
        .to_string(),
    ];

    fs::write(&file_path, lines.join("\n") + "\n")?;
    let parsed = chrono::DateTime::parse_from_rfc3339(meta_rfc3339)?.with_timezone(&chrono::Utc);
    let times = FileTimes::new().set_modified(parsed.into());
    std::fs::OpenOptions::new()
        .append(true)
        .open(&file_path)?
        .set_times(times)?;
    Ok(uuid_str)
}

pub fn create_fake_rollout_with_text_elements(
    codex_home: &Path,
    filename_ts: &str,
    meta_rfc3339: &str,
    preview: &str,
    text_elements: Vec<serde_json::Value>,
    model_provider: Option<&str>,
    git_info: Option<GitInfo>,
) -> Result<String> {
    let uuid = Uuid::new_v4();
    let uuid_str = uuid.to_string();
    let conversation_id = ThreadId::from_string(&uuid_str)?;

    // sessions/YYYY/MM/DD derived from filename_ts (YYYY-MM-DDThh-mm-ss)
    let year = &filename_ts[0..4];
    let month = &filename_ts[5..7];
    let day = &filename_ts[8..10];
    let dir = codex_home.join("sessions").join(year).join(month).join(day);
    fs::create_dir_all(&dir)?;

    let file_path = dir.join(format!("rollout-{filename_ts}-{uuid}.jsonl"));

    // Build JSONL lines
    let meta = SessionMeta {
        id: conversation_id,
        forked_from_id: None,
        timestamp: meta_rfc3339.to_string(),
        cwd: PathBuf::from("/"),
        originator: "codex".to_string(),
        cli_version: "0.0.0".to_string(),
        source: SessionSource::Cli,
        model_provider: model_provider.map(str::to_string),
        base_instructions: None,
    };
    let payload = serde_json::to_value(SessionMetaLine {
        meta,
        git: git_info,
    })?;

    let lines = [
        json!( {
            "timestamp": meta_rfc3339,
            "type": "session_meta",
            "payload": payload
        })
        .to_string(),
        json!( {
            "timestamp": meta_rfc3339,
            "type":"response_item",
            "payload": {
                "type":"message",
                "role":"user",
                "content":[{"type":"input_text","text": preview}]
            }
        })
        .to_string(),
        json!( {
            "timestamp": meta_rfc3339,
            "type":"event_msg",
            "payload": {
                "type":"user_message",
                "message": preview,
                "text_elements": text_elements,
                "local_images": []
            }
        })
        .to_string(),
    ];

    fs::write(file_path, lines.join("\n") + "\n")?;
    Ok(uuid_str)
}
