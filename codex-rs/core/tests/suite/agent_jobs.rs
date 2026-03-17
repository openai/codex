use anyhow::Result;
use codex_core::features::Feature;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
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
    spawn_tool_name: String,
    spawn_args_json: String,
    seen_main: AtomicBool,
    call_counter: AtomicUsize,
}

impl AgentJobsResponder {
    fn new(spawn_tool_name: &str, spawn_args_json: String) -> Self {
        Self {
            spawn_tool_name: spawn_tool_name.to_string(),
            spawn_args_json,
            seen_main: AtomicBool::new(false),
            call_counter: AtomicUsize::new(0),
        }
    }
}

struct StopAfterFirstResponder {
    spawn_tool_name: String,
    spawn_args_json: String,
    seen_main: AtomicBool,
    worker_calls: Arc<AtomicUsize>,
}

impl StopAfterFirstResponder {
    fn new(spawn_tool_name: &str, spawn_args_json: String, worker_calls: Arc<AtomicUsize>) -> Self {
        Self {
            spawn_tool_name: spawn_tool_name.to_string(),
            spawn_args_json,
            seen_main: AtomicBool::new(false),
            worker_calls,
        }
    }
}

#[derive(Clone, Copy)]
enum QueueResponderMode {
    ReportOnly,
    EnqueueThenReport,
    EnqueueTwiceThenReport,
    EnqueueWithDuplicatesThenReport,
    EnqueueWithoutReport,
    StopAfterFirst,
}

struct QueueAgentJobsResponder {
    spawn_args_json: String,
    seen_main: AtomicBool,
    call_counter: AtomicUsize,
    worker_counter: AtomicUsize,
    mode: QueueResponderMode,
}

impl QueueAgentJobsResponder {
    fn new(spawn_args_json: String, mode: QueueResponderMode) -> Self {
        Self {
            spawn_args_json,
            seen_main: AtomicBool::new(false),
            call_counter: AtomicUsize::new(0),
            worker_counter: AtomicUsize::new(0),
            mode,
        }
    }

    fn next_call_id(&self, prefix: &str) -> String {
        let index = self.call_counter.fetch_add(1, Ordering::SeqCst);
        format!("{prefix}-{index}")
    }
}

impl Respond for StopAfterFirstResponder {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let body_bytes = decode_body_bytes(request);
        let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);

        if has_function_call_output(&body) {
            return completed_response("resp-tool");
        }

        if let Some((job_id, item_id)) = extract_job_and_item(&body) {
            let call_index = self.worker_calls.fetch_add(1, Ordering::SeqCst);
            let call_id = format!("call-worker-{call_index}");
            return response_with_events(
                "resp-worker",
                vec![report_result_event(
                    &call_id,
                    &job_id,
                    &item_id,
                    Some(call_index == 0),
                )],
            );
        }

        if !self.seen_main.swap(true, Ordering::SeqCst) {
            return response_with_events(
                "resp-main",
                vec![ev_function_call(
                    "call-spawn",
                    self.spawn_tool_name.as_str(),
                    &self.spawn_args_json,
                )],
            );
        }

        completed_response("resp-default")
    }
}

impl Respond for AgentJobsResponder {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let body_bytes = decode_body_bytes(request);
        let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);

        if has_function_call_output(&body) {
            return completed_response("resp-tool");
        }

        if let Some((job_id, item_id)) = extract_job_and_item(&body) {
            let call_id = format!(
                "call-worker-{}",
                self.call_counter.fetch_add(1, Ordering::SeqCst)
            );
            return response_with_events(
                "resp-worker",
                vec![report_result_event(&call_id, &job_id, &item_id, None)],
            );
        }

        if !self.seen_main.swap(true, Ordering::SeqCst) {
            return response_with_events(
                "resp-main",
                vec![ev_function_call(
                    "call-spawn",
                    self.spawn_tool_name.as_str(),
                    &self.spawn_args_json,
                )],
            );
        }

        completed_response("resp-default")
    }
}

impl Respond for QueueAgentJobsResponder {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let body_bytes = decode_body_bytes(request);
        let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);

        if has_function_call_output(&body) {
            return completed_response("resp-tool");
        }

        if let Some((job_id, item_id)) = extract_job_and_item(&body) {
            let mut events = Vec::new();
            match self.mode {
                QueueResponderMode::ReportOnly => {
                    let call_id = self.next_call_id("call-report");
                    events.push(report_result_event(&call_id, &job_id, &item_id, None));
                }
                QueueResponderMode::EnqueueThenReport => {
                    if item_id == "root" {
                        let enqueue_call_id = self.next_call_id("call-enqueue");
                        events.push(enqueue_items_event(
                            &enqueue_call_id,
                            &job_id,
                            &item_id,
                            vec![
                                json!({
                                    "item_id": "child-1",
                                    "dedupe_key": "https://child-1.test",
                                    "input": { "url": "https://child-1.test" }
                                }),
                                json!({
                                    "item_id": "child-2",
                                    "dedupe_key": "https://child-2.test",
                                    "input": { "url": "https://child-2.test" }
                                }),
                            ],
                        ));
                    }
                    let report_call_id = self.next_call_id("call-report");
                    events.push(report_result_event(
                        &report_call_id,
                        &job_id,
                        &item_id,
                        None,
                    ));
                }
                QueueResponderMode::EnqueueTwiceThenReport => {
                    if item_id == "root" {
                        let enqueue_call_id = self.next_call_id("call-enqueue");
                        events.push(enqueue_items_event(
                            &enqueue_call_id,
                            &job_id,
                            &item_id,
                            vec![json!({
                                "item_id": "child-1",
                                "dedupe_key": "https://child-1.test",
                                "input": { "url": "https://child-1.test" }
                            })],
                        ));
                        let second_enqueue_call_id = self.next_call_id("call-enqueue");
                        events.push(enqueue_items_event(
                            &second_enqueue_call_id,
                            &job_id,
                            &item_id,
                            vec![json!({
                                "item_id": "child-2",
                                "dedupe_key": "https://child-2.test",
                                "input": { "url": "https://child-2.test" }
                            })],
                        ));
                    }
                    let report_call_id = self.next_call_id("call-report");
                    events.push(report_result_event(
                        &report_call_id,
                        &job_id,
                        &item_id,
                        None,
                    ));
                }
                QueueResponderMode::EnqueueWithDuplicatesThenReport => {
                    if item_id == "root" {
                        let enqueue_call_id = self.next_call_id("call-enqueue");
                        events.push(enqueue_items_event(
                            &enqueue_call_id,
                            &job_id,
                            &item_id,
                            vec![
                                json!({
                                    "item_id": "child-1",
                                    "dedupe_key": "https://child-1.test",
                                    "input": { "url": "https://child-1.test" }
                                }),
                                json!({
                                    "item_id": "child-dupe",
                                    "dedupe_key": "https://child-1.test",
                                    "input": { "url": "https://child-dupe.test" }
                                }),
                                json!({
                                    "item_id": "child-2",
                                    "dedupe_key": "https://child-2.test",
                                    "input": { "url": "https://child-2.test" }
                                }),
                            ],
                        ));
                    }
                    let report_call_id = self.next_call_id("call-report");
                    events.push(report_result_event(
                        &report_call_id,
                        &job_id,
                        &item_id,
                        None,
                    ));
                }
                QueueResponderMode::EnqueueWithoutReport => {
                    if item_id == "root" {
                        let enqueue_call_id = self.next_call_id("call-enqueue");
                        events.push(enqueue_items_event(
                            &enqueue_call_id,
                            &job_id,
                            &item_id,
                            vec![
                                json!({
                                    "item_id": "child-1",
                                    "dedupe_key": "https://child-1.test",
                                    "input": { "url": "https://child-1.test" }
                                }),
                                json!({
                                    "item_id": "child-2",
                                    "dedupe_key": "https://child-2.test",
                                    "input": { "url": "https://child-2.test" }
                                }),
                            ],
                        ));
                    } else {
                        let report_call_id = self.next_call_id("call-report");
                        events.push(report_result_event(
                            &report_call_id,
                            &job_id,
                            &item_id,
                            None,
                        ));
                    }
                }
                QueueResponderMode::StopAfterFirst => {
                    let worker_index = self.worker_counter.fetch_add(1, Ordering::SeqCst);
                    let report_call_id = self.next_call_id("call-report");
                    events.push(report_result_event(
                        &report_call_id,
                        &job_id,
                        &item_id,
                        Some(worker_index == 0),
                    ));
                }
            }
            return response_with_events("resp-worker", events);
        }

        if !self.seen_main.swap(true, Ordering::SeqCst) {
            return response_with_events(
                "resp-main",
                vec![ev_function_call(
                    "call-spawn",
                    "spawn_agents_on_queue",
                    &self.spawn_args_json,
                )],
            );
        }

        completed_response("resp-default")
    }
}

fn response_with_events(response_id: &str, mut events: Vec<Value>) -> ResponseTemplate {
    let mut full_events = Vec::with_capacity(events.len() + 2);
    full_events.push(ev_response_created(response_id));
    full_events.append(&mut events);
    full_events.push(ev_completed(response_id));
    sse_response(sse(full_events))
}

fn completed_response(response_id: &str) -> ResponseTemplate {
    response_with_events(response_id, Vec::new())
}

fn report_result_event(call_id: &str, job_id: &str, item_id: &str, stop: Option<bool>) -> Value {
    let mut args = json!({
        "job_id": job_id,
        "item_id": item_id,
        "result": { "item_id": item_id },
    });
    if let Some(stop) = stop
        && let Some(object) = args.as_object_mut()
    {
        object.insert("stop".to_string(), Value::Bool(stop));
    }
    ev_function_call(call_id, "report_agent_job_result", &serialize_json(&args))
}

fn enqueue_items_event(
    call_id: &str,
    job_id: &str,
    parent_item_id: &str,
    items: Vec<Value>,
) -> Value {
    let args = json!({
        "job_id": job_id,
        "parent_item_id": parent_item_id,
        "items": items,
    });
    ev_function_call(call_id, "enqueue_agent_job_items", &serialize_json(&args))
}

fn serialize_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|err| panic!("json serialization failed: {err}"))
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
    let mut combined = texts.join(
        "
",
    );
    if let Some(instructions) = body.get("instructions").and_then(Value::as_str) {
        combined.push('\n');
        combined.push_str(instructions);
    }
    if !combined.contains("You are processing one item for a generic agent job.")
        && !combined.contains("You are processing one item in a queue-draining agent job.")
    {
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

fn parse_jsonl_lines(path: &std::path::Path) -> Result<Vec<Value>> {
    let content = fs::read_to_string(path)?;
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn report_agent_job_result_rejects_wrong_thread() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::SpawnCsv)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
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

    let responder = AgentJobsResponder::new("spawn_agents_on_csv", args_json);
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
        config
            .features
            .enable(Feature::SpawnCsv)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
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

    let responder = AgentJobsResponder::new("spawn_agents_on_csv", args_json);
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
        config
            .features
            .enable(Feature::SpawnCsv)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
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

    let responder = AgentJobsResponder::new("spawn_agents_on_csv", args_json);
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
        config
            .features
            .enable(Feature::SpawnCsv)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
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
    let responder =
        StopAfterFirstResponder::new("spawn_agents_on_csv", args_json, worker_calls.clone());
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agents_on_queue_runs_and_exports_jsonl() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::SpawnCsv)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let output_path = test.cwd_path().join("agent_queue_output.jsonl");
    let args = json!({
        "seed_items": [
            {
                "item_id": "seed-1",
                "dedupe_key": "https://seed-1.test",
                "input": { "url": "https://seed-1.test" }
            },
            {
                "item_id": "seed-2",
                "dedupe_key": "https://seed-2.test",
                "input": { "url": "https://seed-2.test" }
            }
        ],
        "instruction": "Visit {url}",
        "output_jsonl_path": output_path.display().to_string(),
    });
    let args_json = serde_json::to_string(&args)?;

    let responder = QueueAgentJobsResponder::new(args_json, QueueResponderMode::ReportOnly);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .mount(&server)
        .await;

    test.submit_turn("run queue job").await?;

    let lines = parse_jsonl_lines(&output_path)?;
    assert_eq!(lines.len(), 2);
    let item_ids: Vec<_> = lines
        .iter()
        .map(|line| line["item_id"].as_str().expect("item_id"))
        .collect();
    assert_eq!(item_ids, vec!["seed-1", "seed-2"]);
    let statuses: Vec<_> = lines
        .iter()
        .map(|line| line["status"].as_str().expect("status"))
        .collect();
    assert_eq!(statuses, vec!["completed", "completed"]);

    let job_id = lines[0]["job_id"].as_str().expect("job_id");
    let db = test.codex.state_db().expect("state db");
    let job = db.get_agent_job(job_id).await?.expect("job");
    assert_eq!(job.kind, codex_state::AgentJobKind::DynamicQueue);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agents_on_queue_supports_seed_path_json_array() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::SpawnCsv)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let seed_path = test.cwd_path().join("agent_queue_seed.json");
    fs::write(
        &seed_path,
        serde_json::to_string(&json!([
            {
                "item_id": "seed-json",
                "input": { "url": "https://seed-json.test" }
            }
        ]))?,
    )?;
    let output_path = test.cwd_path().join("agent_queue_seed_array.jsonl");
    let args = json!({
        "seed_path": seed_path.display().to_string(),
        "instruction": "Visit {url}",
        "output_jsonl_path": output_path.display().to_string(),
    });
    let args_json = serde_json::to_string(&args)?;

    let responder = QueueAgentJobsResponder::new(args_json, QueueResponderMode::ReportOnly);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .mount(&server)
        .await;

    test.submit_turn("run queue job from json array").await?;

    let lines = parse_jsonl_lines(&output_path)?;
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["item_id"].as_str(), Some("seed-json"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agents_on_queue_supports_seed_path_jsonl() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::SpawnCsv)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let seed_path = test.cwd_path().join("agent_queue_seed.jsonl");
    fs::write(
        &seed_path,
        format!(
            "{}

{}
",
            serialize_json(&json!({
                "item_id": "seed-jsonl-1",
                "input": { "url": "https://seed-jsonl-1.test" }
            })),
            serialize_json(&json!({
                "item_id": "seed-jsonl-2",
                "input": { "url": "https://seed-jsonl-2.test" }
            })),
        ),
    )?;
    let output_path = test.cwd_path().join("agent_queue_seed_jsonl.jsonl");
    let args = json!({
        "seed_path": seed_path.display().to_string(),
        "instruction": "Visit {url}",
        "output_jsonl_path": output_path.display().to_string(),
    });
    let args_json = serde_json::to_string(&args)?;

    let responder = QueueAgentJobsResponder::new(args_json, QueueResponderMode::ReportOnly);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .mount(&server)
        .await;

    test.submit_turn("run queue job from jsonl").await?;

    let lines = parse_jsonl_lines(&output_path)?;
    assert_eq!(lines.len(), 2);
    let item_ids: Vec<_> = lines
        .iter()
        .map(|line| line["item_id"].as_str().expect("item_id"))
        .collect();
    assert_eq!(item_ids, vec!["seed-jsonl-1", "seed-jsonl-2"]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_worker_can_enqueue_more_items() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::SpawnCsv)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let output_path = test.cwd_path().join("agent_queue_children.jsonl");
    let args = json!({
        "seed_items": [{
            "item_id": "root",
            "input": { "url": "https://root.test" }
        }],
        "instruction": "Visit {url}",
        "output_jsonl_path": output_path.display().to_string(),
    });
    let args_json = serde_json::to_string(&args)?;

    let responder = QueueAgentJobsResponder::new(args_json, QueueResponderMode::EnqueueThenReport);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .mount(&server)
        .await;

    test.submit_turn("run expanding queue job").await?;

    let lines = parse_jsonl_lines(&output_path)?;
    assert_eq!(lines.len(), 3);
    let item_ids: Vec<_> = lines
        .iter()
        .map(|line| line["item_id"].as_str().expect("item_id"))
        .collect();
    assert_eq!(item_ids, vec!["root", "child-1", "child-2"]);
    let statuses: Vec<_> = lines
        .iter()
        .map(|line| line["status"].as_str().expect("status"))
        .collect();
    assert_eq!(statuses, vec!["completed", "completed", "completed"]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_worker_multiple_enqueue_calls_are_supported() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::SpawnCsv)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let output_path = test.cwd_path().join("agent_queue_multiple_enqueue.jsonl");
    let args = json!({
        "seed_items": [{
            "item_id": "root",
            "input": { "url": "https://root.test" }
        }],
        "instruction": "Visit {url}",
        "output_jsonl_path": output_path.display().to_string(),
    });
    let args_json = serde_json::to_string(&args)?;

    let responder =
        QueueAgentJobsResponder::new(args_json, QueueResponderMode::EnqueueTwiceThenReport);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .mount(&server)
        .await;

    test.submit_turn("run multi-enqueue queue job").await?;

    let lines = parse_jsonl_lines(&output_path)?;
    let mut item_ids: Vec<_> = lines
        .iter()
        .map(|line| line["item_id"].as_str().expect("item_id"))
        .collect();
    item_ids.sort_unstable();
    assert_eq!(item_ids, vec!["child-1", "child-2", "root"]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_dedupe_key_skips_duplicate_urls() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::SpawnCsv)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let output_path = test.cwd_path().join("agent_queue_dedupe.jsonl");
    let args = json!({
        "seed_items": [{
            "item_id": "root",
            "input": { "url": "https://root.test" }
        }],
        "instruction": "Visit {url}",
        "output_jsonl_path": output_path.display().to_string(),
    });
    let args_json = serde_json::to_string(&args)?;

    let responder = QueueAgentJobsResponder::new(
        args_json,
        QueueResponderMode::EnqueueWithDuplicatesThenReport,
    );
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .mount(&server)
        .await;

    test.submit_turn("run deduping queue job").await?;

    let lines = parse_jsonl_lines(&output_path)?;
    assert_eq!(lines.len(), 3);
    let item_ids: Vec<_> = lines
        .iter()
        .map(|line| line["item_id"].as_str().expect("item_id"))
        .collect();
    assert_eq!(item_ids, vec!["root", "child-1", "child-2"]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_parent_failure_keeps_children() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::SpawnCsv)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let output_path = test.cwd_path().join("agent_queue_parent_failure.jsonl");
    let args = json!({
        "seed_items": [{
            "item_id": "root",
            "input": { "url": "https://root.test" }
        }],
        "instruction": "Visit {url}",
        "output_jsonl_path": output_path.display().to_string(),
    });
    let args_json = serde_json::to_string(&args)?;

    let responder =
        QueueAgentJobsResponder::new(args_json, QueueResponderMode::EnqueueWithoutReport);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .mount(&server)
        .await;

    test.submit_turn("run queue parent failure job").await?;

    let lines = parse_jsonl_lines(&output_path)?;
    assert_eq!(lines.len(), 3);
    let root = lines
        .iter()
        .find(|line| line["item_id"].as_str() == Some("root"))
        .expect("root item");
    assert_eq!(root["status"].as_str(), Some("failed"));
    assert_eq!(
        root["last_error"].as_str(),
        Some("worker finished without calling report_agent_job_result")
    );
    let child_statuses: Vec<_> = lines
        .iter()
        .filter(|line| line["item_id"].as_str() != Some("root"))
        .map(|line| line["status"].as_str().expect("status"))
        .collect();
    assert_eq!(child_statuses, vec!["completed", "completed"]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_stop_cancels_remaining_pending_items() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::SpawnCsv)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let output_path = test.cwd_path().join("agent_queue_stop.jsonl");
    let args = json!({
        "seed_items": [
            { "item_id": "seed-1", "input": { "url": "https://seed-1.test" } },
            { "item_id": "seed-2", "input": { "url": "https://seed-2.test" } },
            { "item_id": "seed-3", "input": { "url": "https://seed-3.test" } }
        ],
        "instruction": "Visit {url}",
        "output_jsonl_path": output_path.display().to_string(),
        "max_concurrency": 1,
    });
    let args_json = serde_json::to_string(&args)?;

    let responder = QueueAgentJobsResponder::new(args_json, QueueResponderMode::StopAfterFirst);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .mount(&server)
        .await;

    test.submit_turn("run stopping queue job").await?;

    let lines = parse_jsonl_lines(&output_path)?;
    assert_eq!(lines.len(), 3);
    let statuses: Vec<_> = lines
        .iter()
        .map(|line| line["status"].as_str().expect("status"))
        .collect();
    assert_eq!(statuses, vec!["completed", "pending", "pending"]);

    let job_id = lines[0]["job_id"].as_str().expect("job_id");
    let db = test.codex.state_db().expect("state db");
    let job = db.get_agent_job(job_id).await?.expect("job");
    assert_eq!(job.status, codex_state::AgentJobStatus::Cancelled);
    let progress = db.get_agent_job_progress(job_id).await?;
    assert_eq!(progress.total_items, 3);
    assert_eq!(progress.completed_items, 1);
    assert_eq!(progress.failed_items, 0);
    assert_eq!(progress.running_items, 0);
    assert_eq!(progress.pending_items, 2);
    Ok(())
}
