use anyhow::Result;
use serde_json::json;
use std::fs;
use std::path::Path;
use uuid::Uuid;

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
) -> Result<String> {
    let uuid = Uuid::new_v4();

    // sessions/YYYY/MM/DD derived from filename_ts (YYYY-MM-DDThh-mm-ss)
    let year = &filename_ts[0..4];
    let month = &filename_ts[5..7];
    let day = &filename_ts[8..10];
    let dir = codex_home.join("sessions").join(year).join(month).join(day);
    fs::create_dir_all(&dir)?;

    let file_path = dir.join(format!("rollout-{filename_ts}-{uuid}.jsonl"));

    // Build JSONL lines
    let mut payload = json!({
        "id": uuid,
        "timestamp": meta_rfc3339,
        "cwd": "/",
        "originator": "codex",
        "cli_version": "0.0.0",
        "instructions": null,
    });
    if let Some(provider) = model_provider {
        payload["model_provider"] = json!(provider);
    }

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

    fs::write(file_path, lines.join("\n") + "\n")?;
    Ok(uuid.to_string())
}
