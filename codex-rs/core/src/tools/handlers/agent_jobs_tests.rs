use super::*;
use chrono::TimeZone;
use chrono::Utc;
use pretty_assertions::assert_eq;
use serde_json::json;

fn sample_queue_job() -> codex_state::AgentJob {
    let now = Utc
        .timestamp_opt(1_700_000_000, 0)
        .single()
        .expect("timestamp");
    codex_state::AgentJob {
        id: "job-1".to_string(),
        name: "queue-job".to_string(),
        kind: codex_state::AgentJobKind::DynamicQueue,
        status: codex_state::AgentJobStatus::Running,
        instruction: "Visit {url}".to_string(),
        auto_export: true,
        max_items: Some(1000),
        max_runtime_seconds: Some(60),
        output_schema_json: Some(json!({
            "type": "object",
            "properties": {
                "item_id": { "type": "string" }
            }
        })),
        input_headers: Vec::new(),
        input_csv_path: String::new(),
        output_csv_path: "/tmp/agent-queue.jsonl".to_string(),
        created_at: now,
        updated_at: now,
        started_at: Some(now),
        completed_at: None,
        last_error: None,
    }
}

fn sample_queue_item() -> codex_state::AgentJobItem {
    let now = Utc
        .timestamp_opt(1_700_000_000, 0)
        .single()
        .expect("timestamp");
    codex_state::AgentJobItem {
        job_id: "job-1".to_string(),
        item_id: "root".to_string(),
        parent_item_id: None,
        row_index: 0,
        source_id: None,
        dedupe_key: Some("https://root.test".to_string()),
        row_json: json!({ "url": "https://root.test" }),
        status: codex_state::AgentJobItemStatus::Completed,
        assigned_thread_id: None,
        attempt_count: 1,
        result_json: Some(json!({ "item_id": "root" })),
        last_error: None,
        created_at: now,
        updated_at: now,
        completed_at: Some(now),
        reported_at: Some(now),
    }
}

#[test]
fn parse_csv_supports_quotes_and_commas() {
    let input = "id,name\n1,\"alpha, beta\"\n2,gamma\n";
    let (headers, rows) = parse_csv(input).expect("csv parse");
    assert_eq!(headers, vec!["id".to_string(), "name".to_string()]);
    assert_eq!(
        rows,
        vec![
            vec!["1".to_string(), "alpha, beta".to_string()],
            vec!["2".to_string(), "gamma".to_string()]
        ]
    );
}

#[test]
fn csv_escape_quotes_when_needed() {
    assert_eq!(csv_escape("simple"), "simple");
    assert_eq!(csv_escape("a,b"), "\"a,b\"");
    assert_eq!(csv_escape("a\"b"), "\"a\"\"b\"");
}

#[test]
fn render_instruction_template_expands_placeholders_and_escapes_braces() {
    let row = json!({
        "path": "src/lib.rs",
        "area": "test",
        "file path": "docs/readme.md",
    });
    let rendered = render_instruction_template(
        "Review {path} in {area}. Also see {file path}. Use {{literal}}.",
        &row,
    );
    assert_eq!(
        rendered,
        "Review src/lib.rs in test. Also see docs/readme.md. Use {literal}."
    );
}

#[test]
fn render_instruction_template_leaves_unknown_placeholders() {
    let row = json!({
        "path": "src/lib.rs",
    });
    let rendered = render_instruction_template("Check {path} then {missing}", &row);
    assert_eq!(rendered, "Check src/lib.rs then {missing}");
}

#[test]
fn ensure_unique_headers_rejects_duplicates() {
    let headers = vec!["path".to_string(), "path".to_string()];
    let Err(err) = ensure_unique_headers(headers.as_slice()) else {
        panic!("expected duplicate header error");
    };
    assert_eq!(
        err,
        FunctionCallError::RespondToModel("csv header path is duplicated".to_string())
    );
}

#[test]
fn parse_queue_seed_content_supports_json_array() {
    let content = r#"[
  {"item_id":"seed-1","input":{"url":"https://seed-1.test"}},
  {"item_id":"seed-2","input":{"url":"https://seed-2.test"}}
]"#;
    let parsed = parse_queue_seed_content(content).expect("queue json array");
    assert_eq!(
        parsed,
        vec![
            QueueJobItemArgs {
                input: json!({ "url": "https://seed-1.test" }),
                item_id: Some("seed-1".to_string()),
                dedupe_key: None,
            },
            QueueJobItemArgs {
                input: json!({ "url": "https://seed-2.test" }),
                item_id: Some("seed-2".to_string()),
                dedupe_key: None,
            },
        ]
    );
}

#[test]
fn parse_queue_seed_content_supports_jsonl() {
    let content = concat!(
        "{\"item_id\":\"seed-1\",\"input\":{\"url\":\"https://seed-1.test\"}}\n",
        "\n",
        "{\"item_id\":\"seed-2\",\"input\":{\"url\":\"https://seed-2.test\"}}\n",
    );
    let parsed = parse_queue_seed_content(content).expect("queue jsonl");
    assert_eq!(
        parsed,
        vec![
            QueueJobItemArgs {
                input: json!({ "url": "https://seed-1.test" }),
                item_id: Some("seed-1".to_string()),
                dedupe_key: None,
            },
            QueueJobItemArgs {
                input: json!({ "url": "https://seed-2.test" }),
                item_id: Some("seed-2".to_string()),
                dedupe_key: None,
            },
        ]
    );
}

#[test]
fn build_initial_queue_job_items_dedupes_and_suffixes_item_ids() {
    let items = vec![
        QueueJobItemArgs {
            input: json!({ "url": "https://a.test" }),
            item_id: Some("dup".to_string()),
            dedupe_key: Some("https://a.test".to_string()),
        },
        QueueJobItemArgs {
            input: json!({ "url": "https://duplicate.test" }),
            item_id: Some("dup".to_string()),
            dedupe_key: Some("https://a.test".to_string()),
        },
        QueueJobItemArgs {
            input: json!({ "url": "https://b.test" }),
            item_id: Some("dup".to_string()),
            dedupe_key: None,
        },
    ];

    let built = build_initial_queue_job_items(items.as_slice(), 10).expect("build queue items");

    assert_eq!(
        built,
        vec![
            codex_state::AgentJobItemCreateParams {
                item_id: "dup".to_string(),
                parent_item_id: None,
                row_index: 0,
                source_id: None,
                dedupe_key: Some("https://a.test".to_string()),
                row_json: json!({ "url": "https://a.test" }),
            },
            codex_state::AgentJobItemCreateParams {
                item_id: "dup-2".to_string(),
                parent_item_id: None,
                row_index: 1,
                source_id: None,
                dedupe_key: None,
                row_json: json!({ "url": "https://b.test" }),
            },
        ]
    );
}

#[test]
fn queue_worker_prompt_mentions_enqueue_tool() {
    let prompt = build_worker_prompt(&sample_queue_job(), &sample_queue_item()).expect("prompt");
    assert!(prompt.contains("queue-draining agent job"));
    assert!(prompt.contains("enqueue_agent_job_items"));
    assert!(prompt.contains("parent_item_id"));
    assert!(prompt.contains("report_agent_job_result"));
}

#[test]
fn render_job_queue_jsonl_outputs_expected_fields() {
    let item = sample_queue_item();
    let rendered = render_job_queue_jsonl(std::slice::from_ref(&item)).expect("render jsonl");
    let line: Value = serde_json::from_str(rendered.trim()).expect("parse jsonl line");
    let timestamp = item.reported_at.expect("reported_at").to_rfc3339();
    assert_eq!(
        line,
        json!({
            "job_id": "job-1",
            "item_id": "root",
            "parent_item_id": null,
            "dedupe_key": "https://root.test",
            "row_index": 0,
            "status": "completed",
            "attempt_count": 1,
            "input": { "url": "https://root.test" },
            "result": { "item_id": "root" },
            "last_error": null,
            "reported_at": timestamp,
            "completed_at": timestamp,
        })
    );
}
