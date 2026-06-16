use super::*;
use crate::session::tests::make_session_and_context;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;

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
fn is_item_stale_allows_second_precision_grace() {
    let now = chrono::Utc::now();
    let item = codex_state::AgentJobItem {
        job_id: "job-1".to_string(),
        item_id: "item-1".to_string(),
        row_index: 0,
        source_id: None,
        row_json: json!({"path": "file-1"}),
        status: codex_state::AgentJobItemStatus::Running,
        assigned_thread_id: Some("00000000-0000-0000-0000-000000000001".to_string()),
        attempt_count: 1,
        result_json: None,
        last_error: None,
        created_at: now,
        updated_at: now - chrono::Duration::milliseconds(1100),
        completed_at: None,
        reported_at: None,
    };

    assert!(!is_item_stale(&item, Duration::from_secs(1)));
}

#[tokio::test]
async fn run_agent_job_loop_fails_stale_state_item_outside_active_map() -> anyhow::Result<()> {
    let (mut session, turn) = make_session_and_context().await;
    let codex_home = tempfile::tempdir()?;
    let db = codex_state::StateRuntime::init(
        codex_home.path().to_path_buf(),
        "test-provider".to_string(),
    )
    .await?;
    session.services.state_db = Some(db.clone());

    let job_id = "job-watchdog".to_string();
    let keepalive_item_id = "keepalive".to_string();
    let target_item_id = "target".to_string();
    let output_path = codex_home.path().join("out.csv");
    db.create_agent_job(
        &codex_state::AgentJobCreateParams {
            id: job_id.clone(),
            name: "test-job".to_string(),
            instruction: "Return {path}".to_string(),
            auto_export: true,
            max_runtime_seconds: Some(1),
            output_schema_json: None,
            input_headers: vec!["path".to_string()],
            input_csv_path: codex_home.path().join("in.csv").display().to_string(),
            output_csv_path: output_path.display().to_string(),
        },
        &[
            codex_state::AgentJobItemCreateParams {
                item_id: keepalive_item_id,
                row_index: 0,
                source_id: None,
                row_json: json!({"path": "keepalive"}),
            },
            codex_state::AgentJobItemCreateParams {
                item_id: target_item_id.clone(),
                row_index: 1,
                source_id: None,
                row_json: json!({"path": "target"}),
            },
        ],
    )
    .await?;
    db.mark_agent_job_running(job_id.as_str()).await?;

    let session = std::sync::Arc::new(session);
    let turn = std::sync::Arc::new(turn);
    let runner = tokio::spawn(run_agent_job_loop(
        std::sync::Arc::clone(&session),
        std::sync::Arc::clone(&turn),
        db.clone(),
        job_id.clone(),
        JobRunnerOptions {
            max_concurrency: 0,
            spawn_config: (*turn.config).clone(),
        },
    ));

    tokio::time::sleep(STATUS_POLL_INTERVAL * 2).await;
    assert!(
        db.mark_agent_job_item_running_with_thread(
            job_id.as_str(),
            target_item_id.as_str(),
            "00000000-0000-0000-0000-000000000001",
        )
        .await?
    );

    let failed_item = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let item = db
                .get_agent_job_item(job_id.as_str(), target_item_id.as_str())
                .await?
                .expect("target item should exist");
            if item.status == codex_state::AgentJobItemStatus::Failed {
                break anyhow::Ok(item);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await??;
    assert_eq!(
        failed_item.last_error,
        Some("worker exceeded max runtime of 1s".to_string())
    );

    assert!(
        db.mark_agent_job_cancelled(job_id.as_str(), "test complete")
            .await?
    );
    let runner_result = tokio::time::timeout(Duration::from_secs(2), runner).await?;
    runner_result??;
    Ok(())
}
