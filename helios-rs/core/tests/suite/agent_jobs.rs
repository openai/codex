use anyhow::Result;
use codex_core::features::Feature;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use wiremock::Mock;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

struct AgentJobsResponder {
    spawn_args_json: String,
    seen_main: AtomicBool,
    call_counter: AtomicUsize,
}

impl AgentJobsResponder {
    fn new(spawn_args_json: String) -> Self {
        Self {
            spawn_args_json,
            seen_main: AtomicBool::new(false),
            call_counter: AtomicUsize::new(0),
        }
    }
}

struct StopAfterFirstResponder {
    spawn_args_json: String,
    seen_main: AtomicBool,
    worker_calls: Arc<AtomicUsize>,
}

impl StopAfterFirstResponder {
    fn new(spawn_args_json: String, worker_calls: Arc<AtomicUsize>) -> Self {
        Self {
            spawn_args_json,
            seen_main: AtomicBool::new(false),
            worker_calls,
        }
    }
}

impl Respond for StopAfterFirstResponder {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let body_bytes = decode_body_bytes(request);
        let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);

        if has_function_call_output(&body) {
            return sse_response(sse(vec![
                ev_response_created("resp-tool"),
                ev_completed("resp-tool"),
            ]));
        }

        if let Some((job_id, item_id)) = extract_job_and_item(&body) {
            let call_index = self.worker_calls.fetch_add(1, Ordering::SeqCst);
            let call_id = format!("call-worker-{call_index}");
            let stop = call_index == 0;
            let args = json!({
                "job_id": job_id,
                "item_id": item_id,
                "result": { "item_id": item_id },
                "stop": stop,
            });
            let args_json = serde_json::to_string(&args).unwrap_or_else(|err| {
                panic!("worker args serialize: {err}");
            });
            return sse_response(sse(vec![
                ev_response_created("resp-worker"),
                ev_function_call(&call_id, "report_agent_job_result", &args_json),
                ev_completed("resp-worker"),
            ]));
        }

        if !self.seen_main.swap(true, Ordering::SeqCst) {
            return sse_response(sse(vec![
                ev_response_created("resp-main"),
                ev_function_call("call-spawn", "spawn_agents_on_csv", &self.spawn_args_json),
                ev_completed("resp-main"),
            ]));
        }

        sse_response(sse(vec![
            ev_response_created("resp-default"),
            ev_completed("resp-default"),
        ]))
    }
}

impl Respond for AgentJobsResponder {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let body_bytes = decode_body_bytes(request);
        let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);

        if has_function_call_output(&body) {
            return sse_response(sse(vec![
                ev_response_created("resp-tool"),
                ev_completed("resp-tool"),
            ]));
        }

        if let Some((job_id, item_id)) = extract_job_and_item(&body) {
            let call_id = format!(
                "call-worker-{}",
                self.call_counter.fetch_add(1, Ordering::SeqCst)
            );
            let args = json!({
                "job_id": job_id,
                "item_id": item_id,
                "result": { "item_id": item_id }
            });
            let args_json = serde_json::to_string(&args).unwrap_or_else(|err| {
                panic!("worker args serialize: {err}");
            });
            return sse_response(sse(vec![
                ev_response_created("resp-worker"),
                ev_function_call(&call_id, "report_agent_job_result", &args_json),
                ev_completed("resp-worker"),
            ]));
        }

        if !self.seen_main.swap(true, Ordering::SeqCst) {
            return sse_response(sse(vec![
                ev_response_created("resp-main"),
                ev_function_call("call-spawn", "spawn_agents_on_csv", &self.spawn_args_json),
                ev_completed("resp-main"),
            ]));
        }

        sse_response(sse(vec![
            ev_response_created("resp-default"),
            ev_completed("resp-default"),
        ]))
    }
}

fn decode_body_bytes(request: &wiremock::Request) -> Vec<u8> {
    let Some(encoding) = request
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
    else {
        return request.body.clone();
    };
    if encoding
        .split(',')
        .any(|entry| entry.trim().eq_ignore_ascii_case("zstd"))
    {
        zstd::stream::decode_all(std::io::Cursor::new(&request.body))
            .unwrap_or_else(|_| request.body.clone())
    } else {
        request.body.clone()
    }
}

fn has_function_call_output(body: &Value) -> bool {
    body.get("input")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("type").and_then(Value::as_str) == Some("function_call_output")
            })
        })
}

fn extract_job_and_item(body: &Value) -> Option<(String, String)> {
    let texts = message_input_texts(body);
    let mut combined = texts.join("\n");
    if let Some(instructions) = body.get("instructions").and_then(Value::as_str) {
        combined.push('\n');
        combined.push_str(instructions);
    }
    if !combined.contains("You are processing one item for a generic agent job.") {
        return None;
    }
    let job_id = Regex::new(r"Job ID:\s*([^\n]+)")
        .ok()?
        .captures(&combined)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().trim().to_string())?;
    let item_id = Regex::new(r"Item ID:\s*([^\n]+)")
        .ok()?
        .captures(&combined)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().trim().to_string())?;
    Some((job_id, item_id))
}

fn message_input_texts(body: &Value) -> Vec<String> {
    let Some(items) = body.get("input").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter(|span| span.get("type").and_then(Value::as_str) == Some("input_text"))
        .filter_map(|span| span.get("text").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn parse_simple_csv_line(line: &str) -> Vec<String> {
    line.split(',').map(str::to_string).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn report_agent_job_result_rejects_wrong_thread() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::Collab);
        config.features.enable(Feature::Sqlite);
    });
    let test = builder.build(&server).await?;

    let input_path = test.cwd_path().join("agent_jobs_wrong_thread.csv");
    let output_path = test.cwd_path().join("agent_jobs_wrong_thread_out.csv");
    fs::write(&input_path, "path\nfile-1\n")?;

    let args = json!({
        "csv_path": input_path.display().to_string(),
        "instruction": "Return {path}",
        "output_csv_path": output_path.display().to_string(),
    });
    let args_json = serde_json::to_string(&args)?;

    let responder = AgentJobsResponder::new(args_json);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .mount(&server)
        .await;

    test.submit_turn("run job").await?;

    let db = test.codex.state_db().expect("state db");
    let output = fs::read_to_string(&output_path)?;
    let rows: Vec<&str> = output.lines().skip(1).collect();
    assert_eq!(rows.len(), 1);
    let job_id = rows
        .first()
        .and_then(|line| {
            parse_simple_csv_line(line)
                .iter()
                .find(|value| value.len() == 36)
                .cloned()
        })
        .expect("job_id from csv");
    let job = db.get_agent_job(job_id.as_str()).await?.expect("job");
    let items = db
        .list_agent_job_items(job.id.as_str(), None, Some(10))
        .await?;
    let item = items.first().expect("item");
    let wrong_thread_id = "00000000-0000-0000-0000-000000000000";
    let accepted = db
        .report_agent_job_item_result(
            job.id.as_str(),
            item.item_id.as_str(),
            wrong_thread_id,
            &json!({ "wrong": true }),
        )
        .await?;
    assert!(!accepted);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agents_on_csv_runs_and_exports() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::Collab);
        config.features.enable(Feature::Sqlite);
    });
    let test = builder.build(&server).await?;

    let input_path = test.cwd_path().join("agent_jobs_input.csv");
    let output_path = test.cwd_path().join("agent_jobs_output.csv");
    fs::write(&input_path, "path,area\nfile-1,test\nfile-2,test\n")?;

    let args = json!({
        "csv_path": input_path.display().to_string(),
        "instruction": "Return {path}",
        "output_csv_path": output_path.display().to_string(),
    });
    let args_json = serde_json::to_string(&args)?;

    let responder = AgentJobsResponder::new(args_json);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .mount(&server)
        .await;

    test.submit_turn("run batch job").await?;

    let output = fs::read_to_string(&output_path)?;
    assert!(output.contains("result_json"));
    assert!(output.contains("item_id"));
    assert!(output.contains("\"item_id\""));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agents_on_csv_dedupes_item_ids() -> Result<()> {
    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::Collab);
        config.features.enable(Feature::Sqlite);
    });
    let test = builder.build(&server).await?;

    let input_path = test.cwd_path().join("agent_jobs_dupe.csv");
    let output_path = test.cwd_path().join("agent_jobs_dupe_out.csv");
    fs::write(&input_path, "id,path\nfoo,alpha\nfoo,beta\n")?;

    let args = json!({
        "csv_path": input_path.display().to_string(),
        "instruction": "Return {path}",
        "id_column": "id",
        "output_csv_path": output_path.display().to_string(),
    });
    let args_json = serde_json::to_string(&args)?;

    let responder = AgentJobsResponder::new(args_json);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .mount(&server)
        .await;

    test.submit_turn("run batch job with duplicate ids").await?;

    let output = fs::read_to_string(&output_path)?;
    let mut lines = output.lines();
    let headers = lines.next().expect("csv headers");
    let header_cols = parse_simple_csv_line(headers);
    let item_id_index = header_cols
        .iter()
        .position(|header| header == "item_id")
        .expect("item_id column");

    let mut item_ids = Vec::new();
    for line in lines {
        let cols = parse_simple_csv_line(line);
        item_ids.push(cols[item_id_index].clone());
    }
    item_ids.sort();
    item_ids.dedup();
    assert_eq!(item_ids.len(), 2);
    assert!(item_ids.contains(&"foo".to_string()));
    assert!(item_ids.contains(&"foo-2".to_string()));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agents_on_csv_stop_halts_future_items() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::Collab);
        config.features.enable(Feature::Sqlite);
    });
    let test = builder.build(&server).await?;

    let input_path = test.cwd_path().join("agent_jobs_stop.csv");
    let output_path = test.cwd_path().join("agent_jobs_stop_out.csv");
    fs::write(&input_path, "path\nfile-1\nfile-2\nfile-3\n")?;

    let args = json!({
        "csv_path": input_path.display().to_string(),
        "instruction": "Return {path}",
        "output_csv_path": output_path.display().to_string(),
        "max_concurrency": 1,
    });
    let args_json = serde_json::to_string(&args)?;

    let worker_calls = Arc::new(AtomicUsize::new(0));
    let responder = StopAfterFirstResponder::new(args_json, worker_calls.clone());
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .mount(&server)
        .await;

    test.submit_turn("run job").await?;

    let output = fs::read_to_string(&output_path)?;
    let rows: Vec<&str> = output.lines().skip(1).collect();
    assert_eq!(rows.len(), 3);
    let job_id = rows
        .first()
        .and_then(|line| {
            parse_simple_csv_line(line)
                .iter()
                .find(|value| value.len() == 36)
                .cloned()
        })
        .expect("job_id from csv");
    let db = test.codex.state_db().expect("state db");
    let job = db.get_agent_job(job_id.as_str()).await?.expect("job");
    assert_eq!(job.status, codex_state::AgentJobStatus::Cancelled);
    let progress = db.get_agent_job_progress(job_id.as_str()).await?;
    assert_eq!(progress.total_items, 3);
    assert_eq!(progress.completed_items, 1);
    assert_eq!(progress.failed_items, 0);
    assert_eq!(progress.running_items, 0);
    assert_eq!(progress.pending_items, 2);
    assert_eq!(worker_calls.load(Ordering::SeqCst), 1);
    Ok(())
}
