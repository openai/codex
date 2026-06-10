use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

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

#[tokio::test]
async fn assigning_thread_before_prompt_delivery_does_not_mark_item_running() -> anyhow::Result<()>
{
    let codex_home = tempfile::tempdir()?;
    let runtime =
        codex_state::StateRuntime::init(codex_home.path().to_path_buf(), "test-provider".into())
            .await?;
    let job_id = "job-before-prompt";
    let item_id = "row-1";

    runtime
        .create_agent_job(
            &codex_state::AgentJobCreateParams {
                id: job_id.to_string(),
                name: "test-job".to_string(),
                instruction: "Return {path}".to_string(),
                auto_export: true,
                max_runtime_seconds: Some(1),
                output_schema_json: None,
                input_headers: vec!["path".to_string()],
                input_csv_path: "/tmp/input.csv".to_string(),
                output_csv_path: "/tmp/output.csv".to_string(),
            },
            &[codex_state::AgentJobItemCreateParams {
                item_id: item_id.to_string(),
                row_index: 0,
                source_id: None,
                row_json: json!({"path": "file-1"}),
            }],
        )
        .await?;
    runtime.mark_agent_job_running(job_id).await?;

    let marked_running = runtime
        .mark_agent_job_item_running_with_thread(job_id, item_id, "child-thread-without-prompt")
        .await?;
    assert!(
        !marked_running,
        "assigning a child thread is not enough to mark an item running; \
         the worker prompt must be delivered first"
    );

    let item = runtime
        .get_agent_job_item(job_id, item_id)
        .await?
        .expect("job item should exist");
    assert_eq!(item.status, codex_state::AgentJobItemStatus::Pending);
    assert_eq!(item.assigned_thread_id, None);
    assert_eq!(
        runtime.get_agent_job_progress(job_id).await?,
        codex_state::AgentJobProgress {
            total_items: 1,
            pending_items: 1,
            running_items: 0,
            completed_items: 0,
            failed_items: 0,
        }
    );

    Ok(())
}

#[tokio::test]
async fn running_item_past_max_runtime_fails_even_if_worker_never_returns() -> anyhow::Result<()> {
    let codex_home = tempfile::tempdir()?;
    let runtime =
        codex_state::StateRuntime::init(codex_home.path().to_path_buf(), "test-provider".into())
            .await?;
    let job_id = "job-timeout";
    let item_id = "row-1";

    runtime
        .create_agent_job(
            &codex_state::AgentJobCreateParams {
                id: job_id.to_string(),
                name: "test-job".to_string(),
                instruction: "Return {path}".to_string(),
                auto_export: true,
                max_runtime_seconds: Some(1),
                output_schema_json: None,
                input_headers: vec!["path".to_string()],
                input_csv_path: "/tmp/input.csv".to_string(),
                output_csv_path: "/tmp/output.csv".to_string(),
            },
            &[codex_state::AgentJobItemCreateParams {
                item_id: item_id.to_string(),
                row_index: 0,
                source_id: None,
                row_json: json!({"path": "file-1"}),
            }],
        )
        .await?;
    runtime.mark_agent_job_running(job_id).await?;
    let marked_running = runtime
        .mark_agent_job_item_running_with_thread(
            job_id,
            item_id,
            "00000000-0000-0000-0000-000000000001",
        )
        .await?;
    assert!(marked_running);

    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;

    let item = runtime
        .get_agent_job_item(job_id, item_id)
        .await?
        .expect("job item should exist");
    assert_eq!(item.status, codex_state::AgentJobItemStatus::Failed);
    assert_eq!(item.assigned_thread_id, None);
    assert!(
        item.last_error
            .as_deref()
            .is_some_and(|error| error.contains("worker exceeded max runtime")),
        "timed out running item should record a max-runtime failure"
    );
    assert_eq!(
        runtime.get_agent_job_progress(job_id).await?,
        codex_state::AgentJobProgress {
            total_items: 1,
            pending_items: 0,
            running_items: 0,
            completed_items: 0,
            failed_items: 1,
        }
    );

    Ok(())
}
