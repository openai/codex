use anyhow::Context;
use anyhow::Result;
use serde_json::Value;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn build_claude_resume_prompt(
    source: &str,
    claude_home: Option<&Path>,
    max_chars: Option<usize>,
) -> Result<String> {
    let session_file = resolve_session_file(source, claude_home)?;
    let transcript = render_transcript(&session_file, max_chars)?;

    let prompt = format!(
        concat!(
            "You are resuming a prior conversation that originally happened in Claude Code.\n",
            "Treat the transcript below as prior context and continue from it.\n",
            "Do not restate the full transcript.\n",
            "\n",
            "SOURCE_SESSION_FILE: {}\n",
            "\n",
            "BEGIN_IMPORTED_TRANSCRIPT\n",
            "{}\n",
            "END_IMPORTED_TRANSCRIPT\n",
        ),
        session_file.display(),
        transcript
    );

    Ok(prompt)
}

fn resolve_session_file(source: &str, claude_home: Option<&Path>) -> Result<PathBuf> {
    let source_path = Path::new(source);
    if source_path.exists() {
        anyhow::ensure!(
            source_path.is_file(),
            "Claude source path must be a file: {}",
            source_path.display()
        );
        return Ok(source_path.to_path_buf());
    }

    let claude_home = if let Some(home) = claude_home {
        home.to_path_buf()
    } else {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is not set; pass --claude-home to locate Claude sessions")?;
        home.join(".claude")
    };

    let projects_dir = claude_home.join("projects");
    anyhow::ensure!(
        projects_dir.is_dir(),
        "Claude projects directory not found: {}",
        projects_dir.display()
    );

    let filename = format!("{source}.jsonl");
    let mut matches = Vec::new();

    for entry in std::fs::read_dir(&projects_dir)
        .with_context(|| format!("failed to read {}", projects_dir.display()))?
    {
        let entry = entry?;
        let project_dir = entry.path();
        if !project_dir.is_dir() {
            continue;
        }

        let direct = project_dir.join(&filename);
        if direct.is_file() {
            matches.push(direct);
        }
    }

    if matches.is_empty() {
        for entry in std::fs::read_dir(&projects_dir)
            .with_context(|| format!("failed to read {}", projects_dir.display()))?
        {
            let entry = entry?;
            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }

            let index_path = project_dir.join("sessions-index.json");
            if !index_path.is_file() {
                continue;
            }

            let file = match File::open(&index_path) {
                Ok(file) => file,
                Err(_) => continue,
            };
            let index_json: Value = match serde_json::from_reader(file) {
                Ok(value) => value,
                Err(_) => continue,
            };

            let Some(entries) = index_json.get("entries").and_then(Value::as_array) else {
                continue;
            };

            for item in entries {
                let Some(session_id) = item.get("sessionId").and_then(Value::as_str) else {
                    continue;
                };
                if session_id != source {
                    continue;
                }
                let Some(full_path) = item.get("fullPath").and_then(Value::as_str) else {
                    continue;
                };
                let p = PathBuf::from(full_path);
                if p.is_file() {
                    matches.push(p);
                }
            }
        }
    }

    match matches.len() {
        0 => anyhow::bail!(
            "could not find Claude session '{source}' under {}",
            projects_dir.display()
        ),
        1 => Ok(matches.remove(0)),
        _ => {
            let paths = matches
                .iter()
                .map(|p| format!("- {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n");
            anyhow::bail!(
                "multiple Claude sessions matched '{source}'; pass a direct path instead:\n{paths}"
            )
        }
    }
}

fn render_transcript(path: &Path, max_chars: Option<usize>) -> Result<String> {
    let file = File::open(path)
        .with_context(|| format!("failed to open session file {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut transcript_lines = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let value: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = value.get("type").and_then(Value::as_str);
        if matches!(
            event_type,
            Some("progress" | "queue-operation" | "file-history-snapshot")
        ) {
            continue;
        }

        let role = match event_type {
            Some("user") => "USER",
            Some("assistant") => "ASSISTANT",
            _ => continue,
        };

        let content = value
            .get("message")
            .and_then(|message| message.get("content"))
            .map(extract_content_text)
            .unwrap_or_default();

        if content.trim().is_empty() {
            continue;
        }

        let timestamp = value
            .get("timestamp")
            .and_then(Value::as_str)
            .map(|ts| format!(" [{ts}]"))
            .unwrap_or_default();
        transcript_lines.push(format!("{role}{timestamp}:\n{content}"));
    }

    let mut transcript = transcript_lines.join("\n\n");
    if let Some(limit) = max_chars {
        transcript = truncate_to_tail_chars(transcript, limit);
    }
    Ok(transcript)
}

fn extract_content_text(value: &Value) -> String {
    if let Some(s) = value.as_str() {
        return s.to_string();
    }

    let Some(items) = value.as_array() else {
        return String::new();
    };

    let mut parts = Vec::new();
    for item in items {
        let Some(item_obj) = item.as_object() else {
            continue;
        };

        let item_type = item_obj.get("type").and_then(Value::as_str).unwrap_or("");
        match item_type {
            "text" => {
                if let Some(text) = item_obj.get("text").and_then(Value::as_str) {
                    parts.push(text.to_string());
                }
            }
            "tool_use" => {
                let tool_name = item_obj
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown_tool");
                let input = item_obj.get("input").cloned().unwrap_or(Value::Null);
                parts.push(format!("[assistant_tool_use:{tool_name}] {input}"));
            }
            "tool_result" => {
                let content = item_obj.get("content").cloned().unwrap_or(Value::Null);
                parts.push(format!("[tool_result] {content}"));
            }
            _ => {}
        }
    }
    parts.join("\n")
}

fn truncate_to_tail_chars(value: String, max_chars: usize) -> String {
    if max_chars == 0 {
        return "[TRUNCATED TO LAST 0 CHARS]\n".to_string();
    }

    let total_chars = value.chars().count();
    if total_chars <= max_chars {
        return value;
    }

    let start = total_chars - max_chars;
    let tail = value.chars().skip(start).collect::<String>();
    format!("[TRUNCATED TO LAST {max_chars} CHARS]\n{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn resolve_session_file_finds_direct_project_jsonl() {
        let dir = tempdir().expect("tempdir");
        let claude_home = dir.path().join(".claude");
        let project = claude_home.join("projects").join("demo-project");
        std::fs::create_dir_all(&project).expect("create project dir");

        let session_id = "11111111-2222-4333-8444-555555555555";
        let session_file = project.join(format!("{session_id}.jsonl"));
        std::fs::write(&session_file, "").expect("write session");

        let found = resolve_session_file(session_id, Some(&claude_home)).expect("resolve session");
        assert_eq!(found, session_file);
    }

    #[test]
    fn render_transcript_keeps_user_and_assistant_events() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("sample.jsonl");
        let content = [
            r#"{"type":"progress","data":{"ignored":true}}"#,
            r#"{"type":"user","timestamp":"2026-02-07T00:00:00Z","message":{"content":"hello from user"}}"#,
            r#"{"type":"assistant","timestamp":"2026-02-07T00:00:01Z","message":{"content":[{"type":"text","text":"hello from assistant"},{"type":"tool_use","name":"Bash","input":{"command":"ls"}}]}}"#,
        ]
        .join("\n");
        std::fs::write(&file, content).expect("write sample");

        let transcript = render_transcript(&file, None).expect("render transcript");
        assert!(transcript.contains("USER [2026-02-07T00:00:00Z]:\nhello from user"));
        assert!(transcript.contains("ASSISTANT [2026-02-07T00:00:01Z]:"));
        assert!(transcript.contains("hello from assistant"));
        assert!(transcript.contains("[assistant_tool_use:Bash] {\"command\":\"ls\"}"));
        assert!(!transcript.contains("ignored"));
    }

    #[test]
    fn truncate_to_tail_chars_truncates_and_marks_output() {
        let out = truncate_to_tail_chars("abcdef".to_string(), 3);
        assert_eq!(out, "[TRUNCATED TO LAST 3 CHARS]\ndef");
    }

    #[test]
    fn build_prompt_accepts_direct_file_path() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("session.jsonl");
        std::fs::write(
            &file,
            r#"{"type":"user","message":{"content":"first prompt from claude"}}"#,
        )
        .expect("write sample");

        let prompt =
            build_claude_resume_prompt(file.to_str().expect("utf-8 path"), None, Some(10_000))
                .expect("build prompt");

        assert!(prompt.contains("BEGIN_IMPORTED_TRANSCRIPT"));
        assert!(prompt.contains("first prompt from claude"));
        assert!(prompt.contains("END_IMPORTED_TRANSCRIPT"));
    }
}
