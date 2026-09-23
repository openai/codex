use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::app_server_json_shutdown_event;
use app_test_support::create_exec_command_sse_response;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::encode_id_token;
use app_test_support::write_models_cache;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use codex_state::LogQuery;
use codex_state::SqliteConfig;
use codex_state::StateRuntime;
use codex_utils_absolute_path::test_support::PathExt;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;
use tempfile::TempDir;
use test_case::test_case;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::time::Duration;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn credentials_stay_out_of_persisted_and_feedback_logs() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let bearer = "synthetic-provider-bearer";
    let header = "synthetic-provider-header";
    let attestation = "synthetic-attestation-token";
    let account_id = "123e4567-e89b-42d3-a456-426614174011";
    let initial_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("initial@example.com")
            .chatgpt_user_id("logging-user")
            .chatgpt_account_id(account_id),
    )?;
    let refreshed_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("refreshed@example.com")
            .chatgpt_user_id("logging-user")
            .chatgpt_account_id(account_id),
    )?;
    let server = MockServer::start().await;
    let success = responses::sse_response(create_final_assistant_message_sse_response("done")?);
    let responses = responses::mount_response_sequence(
        &server,
        vec![success.clone(), ResponseTemplate::new(401), success],
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/settings/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "commit_attribution_enabled": false,
        })))
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_uri = server.uri();
    MockResponsesConfig::new(&server_uri)
        .with_root_config(&format!("chatgpt_base_url = \"{server_uri}/backend-api\""))
        .with_provider_config("requires_openai_auth = true\nsupports_websockets = false")
        .with_extra_config(&format!(
            r#"
[model_providers.bearer_provider]
name = "Bearer provider"
base_url = "{server_uri}/v1"
experimental_bearer_token = "{bearer}"
http_headers = {{ X-Credential = "{header}" }}
supports_websockets = false
"#
        ))
        .write(codex_home.path())?;
    write_models_cache(codex_home.path()).await?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build()
        .await?;
    let initialized = app_server
        .initialize_with_capabilities(
            ClientInfo {
                name: "codex_desktop".into(),
                title: None,
                version: "0.1.0".into(),
            },
            Some(InitializeCapabilities {
                experimental_api: true,
                request_attestation: true,
                ..Default::default()
            }),
        )
        .await?;
    anyhow::ensure!(
        matches!(initialized, JSONRPCMessage::Response(_)),
        "initialization failed"
    );
    let login_id = app_server
        .send_chatgpt_auth_tokens_login_request(
            initial_token.clone(),
            account_id.into(),
            Some("pro".into()),
        )
        .await?;
    let _: LoginAccountResponse = app_server.read_response(login_id).await?;

    let mut thread_ids = Vec::new();
    for provider in ["bearer_provider", "mock_provider"] {
        let thread = app_server
            .start_thread(ThreadStartParams {
                model_provider: Some(provider.into()),
                ..Default::default()
            })
            .await?
            .thread;
        app_server
            .send_turn_start_request(TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "hello".into(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        timeout(Duration::from_secs(/*secs*/ 60), async {
            loop {
                match app_server.read_next_message().await? {
                    JSONRPCMessage::Request(request) => {
                        let (request_id, result) = match ServerRequest::try_from(request)? {
                            ServerRequest::AttestationGenerate { request_id, .. } => {
                                (request_id, json!({ "token": attestation }))
                            }
                            ServerRequest::ChatgptAuthTokensRefresh { request_id, .. } => (
                                request_id,
                                json!({
                                    "accessToken": refreshed_token,
                                    "chatgptAccountId": account_id,
                                    "chatgptPlanType": "pro",
                                }),
                            ),
                            request => anyhow::bail!("unexpected request: {request:?}"),
                        };
                        app_server.send_response(request_id, result).await?;
                    }
                    JSONRPCMessage::Notification(notification)
                        if notification.method == "turn/completed" =>
                    {
                        let params = notification
                            .params
                            .context("missing turn/completed params")?;
                        assert_eq!(params["turn"]["status"], "completed");
                        break Ok::<_, anyhow::Error>(());
                    }
                    JSONRPCMessage::Error(error) => anyhow::bail!("unexpected error: {error:?}"),
                    JSONRPCMessage::Response(_) | JSONRPCMessage::Notification(_) => {}
                }
            }
        })
        .await??;
        thread_ids.push(thread.id);
    }
    let requests = responses.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.header("authorization"))
            .collect::<Vec<_>>(),
        vec![
            Some(format!("Bearer {bearer}")),
            Some(format!("Bearer {initial_token}")),
            Some(format!("Bearer {refreshed_token}")),
        ]
    );
    assert_eq!(requests[0].header("x-credential").as_deref(), Some(header));
    assert_eq!(
        requests[2].header("x-oai-attestation"),
        Some(format!(r#"{{"v":1,"s":0,"t":"{attestation}"}}"#))
    );

    // Wait for a later event so buffered logs cannot hide a leak.
    let barrier = "credential-log-barrier";
    app_server
        .send_response(RequestId::String(barrier.into()), json!({}))
        .await?;
    let state = StateRuntime::init(
        SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;
    let thread_ids = thread_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let feedback = timeout(Duration::from_secs(/*secs*/ 60), async {
        loop {
            let logs =
                String::from_utf8(state.query_feedback_logs_for_threads(&thread_ids).await?)?;
            if logs.contains(barrier) {
                break Ok::<_, anyhow::Error>(logs);
            }
            tokio::time::sleep(Duration::from_millis(/*millis*/ 50)).await;
        }
    })
    .await??;
    let persisted = format!("{:?}", state.query_logs(&LogQuery::default()).await?);
    state.close().await;
    // The HTTP assertions above prove the credentials were used. The barrier
    // confirms earlier queued logs were persisted before we check for leaks.
    for (sink, logs) in [("SQLite", persisted), ("feedback", feedback)] {
        anyhow::ensure!(logs.contains(barrier), "missing log barrier in {sink} logs");
        for secret in [
            bearer,
            header,
            &initial_token,
            &refreshed_token,
            attestation,
        ] {
            anyhow::ensure!(
                !logs.contains(secret),
                "credential leaked into {sink} logs: {secret}"
            );
        }
    }
    Ok(())
}

#[test]
fn standalone_app_server_emits_json_info_events() -> Result<()> {
    let codex_home = TempDir::new()?;
    let event = app_server_json_shutdown_event("codex-app-server", &[], codex_home.path())?;

    assert_eq!(
        event,
        json!({
            "level": "INFO",
            "fields": {
                "message": "processor task exited",
                "exit_reason": "stdio_connection_closed",
                "remaining_connection_count": 0,
                "shutdown_forced": false,
            },
            "target": "codex_app_server",
        })
    );

    Ok(())
}

#[tokio::test]
async fn sqlite_log_metrics_exports_do_not_create_log_cycles() -> Result<()> {
    let quiet_period = codex_state::log_db::LogSinkQueueConfig::default().flush_interval
        + Duration::from_millis(600);
    let export_timeout = quiet_period * 3;

    for status in [200, 500] {
        let collector = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/metrics"))
            .respond_with(ResponseTemplate::new(status))
            .mount(&collector)
            .await;

        let codex_home = TempDir::new()?;
        let endpoint = format!("{}/v1/metrics", collector.uri());
        std::fs::write(
            codex_home.path().join("config.toml"),
            format!(
                "[analytics]\nenabled = true\n\n[otel.metrics_exporter.otlp-http]\nendpoint = {endpoint:?}\nprotocol = \"json\"\n"
            ),
        )?;

        let _app_server = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .with_env_overrides(&[
                ("OTEL_METRIC_EXPORT_INTERVAL", Some("200")),
                (
                    codex_app_server_transport::REMOTE_CONTROL_DISABLED_ENV_VAR,
                    Some("1"),
                ),
            ])
            .build_initialized_with_timeout(export_timeout)
            .await?;

        let exports = timeout(export_timeout, async {
            loop {
                let requests = collector.received_requests().await.unwrap_or_default();
                if requests.iter().any(|request| {
                    String::from_utf8_lossy(&request.body).contains(codex_state::LOG_WRITE_METRIC)
                }) {
                    break requests.len();
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await?;

        let requests = timeout(export_timeout, async {
            let mut observed_exports = exports;
            loop {
                tokio::time::sleep(quiet_period).await;
                let requests = collector.received_requests().await.unwrap_or_default();
                if requests.len() == observed_exports {
                    break requests;
                }
                observed_exports = requests.len();
            }
        })
        .await
        .with_context(|| {
            format!("OTLP HTTP {status} exports did not stop after the application became idle")
        })?;

        let expected_metrics = [
            (codex_state::LOG_WRITE_METRIC, "sum"),
            (codex_state::LOG_WRITE_DURATION_METRIC, "histogram"),
            (codex_state::LOG_WRITE_BYTES_METRIC, "histogram"),
            (codex_state::LOG_WRITE_ENTRIES_METRIC, "histogram"),
            (codex_state::LOG_WRITE_MAX_ENTRY_BYTES_METRIC, "histogram"),
        ];
        let payloads = requests
            .iter()
            .map(|request| serde_json::from_slice::<Value>(&request.body))
            .collect::<serde_json::Result<Vec<_>>>()?;
        let mut observed_metrics = BTreeMap::<&str, (u64, f64)>::new();
        for (name, kind) in expected_metrics {
            let points = payloads
                .iter()
                .flat_map(|payload| payload["resourceMetrics"].as_array().into_iter().flatten())
                .flat_map(|resource| resource["scopeMetrics"].as_array().into_iter().flatten())
                .flat_map(|scope| scope["metrics"].as_array().into_iter().flatten())
                .filter(|metric| metric["name"].as_str() == Some(name))
                .flat_map(|metric| metric[kind]["dataPoints"].as_array().into_iter().flatten())
                .collect::<Vec<_>>();
            assert!(
                !points.is_empty(),
                "metric {name} must reach the collector as a {kind}"
            );

            for point in points {
                let attributes: BTreeMap<&str, &str> = point["attributes"]
                    .as_array()
                    .context("metric data points must include attributes")?
                    .iter()
                    .filter_map(|attribute| {
                        Some((
                            attribute["key"].as_str()?,
                            attribute["value"]["stringValue"].as_str()?,
                        ))
                    })
                    .collect();
                assert_eq!(
                    attributes,
                    BTreeMap::from([
                        ("error", "none"),
                        ("originator", "codex-app-server"),
                        ("status", "success"),
                    ]),
                    "metric {name} must preserve its production dimensions"
                );

                let count_field = if kind == "sum" { "asInt" } else { "count" };
                let count = point[count_field]
                    .as_u64()
                    .or_else(|| point[count_field].as_str()?.parse().ok())
                    .with_context(|| format!("metric {name} must include a valid {count_field}"))?;
                let value = if kind == "sum" {
                    count as f64
                } else {
                    point["sum"]
                        .as_f64()
                        .with_context(|| format!("metric {name} must include its sample sum"))?
                };
                let observed = observed_metrics.entry(name).or_default();
                observed.0 += count;
                observed.1 += value;

                if matches!(
                    name,
                    codex_state::LOG_WRITE_BYTES_METRIC
                        | codex_state::LOG_WRITE_MAX_ENTRY_BYTES_METRIC
                ) {
                    let bounds = point["explicitBounds"]
                        .as_array()
                        .context("byte histograms must export explicit bucket bounds")?;
                    assert_eq!(bounds.first().and_then(Value::as_f64), Some(128.0));
                    assert_eq!(bounds.last().and_then(Value::as_f64), Some(16_777_216.0));
                }
            }
        }

        let write_count = observed_metrics[codex_state::LOG_WRITE_METRIC].0;
        assert!(write_count > 0, "at least one SQLite batch must be written");
        for (name, _) in expected_metrics {
            assert_eq!(
                observed_metrics[name].0, write_count,
                "metric {name} must record exactly one sample per SQLite write"
            );
        }

        let state = codex_state::StateRuntime::init(
            codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
            "test-provider".to_string(),
        )
        .await?;
        let persisted_logs = state.query_logs(&codex_state::LogQuery::default()).await?;
        assert_eq!(
            observed_metrics[codex_state::LOG_WRITE_ENTRIES_METRIC].1,
            persisted_logs.len() as f64,
            "exported batch sizes must equal the number of persisted SQLite log rows"
        );
        assert!(
            observed_metrics[codex_state::LOG_WRITE_BYTES_METRIC].1
                >= observed_metrics[codex_state::LOG_WRITE_MAX_ENTRY_BYTES_METRIC].1,
            "each batch's largest entry cannot exceed the total batch size"
        );
    }

    Ok(())
}

/// Exporting SQLite metrics must eventually stop producing new SQLite metrics,
/// including when the collector uses HTTP/2 or rejects the gRPC request.
#[test_case("0"; "success")]
#[test_case("14"; "unavailable")]
#[tokio::test]
async fn sqlite_log_metrics_grpc_exports_do_not_create_log_cycles(grpc_status: &str) -> Result<()> {
    let quiet_period = codex_state::log_db::LogSinkQueueConfig::default().flush_interval
        + Duration::from_millis(600);
    let export_timeout = quiet_period * 3;
    let (export_count, mut exports) = watch::channel(/*init*/ 0_usize);
    let (ping_ack_count, mut ping_acknowledgements) = watch::channel(/*init*/ 0_usize);
    let collector = MockServer::builder()
        .disable_request_recording()
        .start()
        .await;
    // An uncompressed gRPC frame containing an empty ExportMetricsServiceResponse.
    // Tonic also accepts grpc-status in the initial response headers.
    let response = ResponseTemplate::new(/*s*/ 200)
        .set_body_raw(vec![0; 5], "application/grpc")
        .insert_header("grpc-status", grpc_status);
    Mock::given(method("POST"))
        .and(path(
            "/opentelemetry.proto.collector.metrics.v1.MetricsService/Export",
        ))
        .respond_with(move |request: &wiremock::Request| {
            let metric_name = codex_state::LOG_WRITE_METRIC.as_bytes();
            if request
                .body
                .windows(metric_name.len())
                .any(|bytes| bytes == metric_name)
            {
                export_count.send_modify(|count| *count += 1);
            }
            response.clone()
        })
        .mount(&collector)
        .await;

    let proxy = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}", proxy.local_addr()?);
    let collector_address = *collector.address();
    let _proxy = tokio::spawn(async move {
        let (downstream, _) = proxy.accept().await?;
        let upstream = TcpStream::connect(collector_address).await?;
        let (mut downstream_reader, mut downstream_writer) = downstream.into_split();
        let (mut upstream_reader, mut upstream_writer) = upstream.into_split();

        let requests = tokio::io::copy(&mut downstream_reader, &mut upstream_writer);
        let responses = async move {
            loop {
                let mut header = [0; 9];
                if upstream_reader.read_exact(&mut header).await.is_err() {
                    break;
                }
                let length = (usize::from(header[0]) << 16)
                    | (usize::from(header[1]) << 8)
                    | usize::from(header[2]);
                let mut payload = vec![0; length];
                upstream_reader.read_exact(&mut payload).await?;
                downstream_writer.write_all(&header).await?;
                downstream_writer.write_all(&payload).await?;

                if header[3] == 0 {
                    // A valid but unsolicited HTTP/2 PING ACK makes h2 emit a
                    // warning from its separate connection-driving task.
                    downstream_writer
                        .write_all(&[0, 0, 8, 6, 1, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8])
                        .await?;
                    ping_ack_count.send_modify(|count| *count += 1);
                }
            }
            Ok::<(), std::io::Error>(())
        };

        tokio::try_join!(requests, responses)?;
        Ok::<(), std::io::Error>(())
    });

    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            "[analytics]\nenabled = true\n\n[otel.metrics_exporter.otlp-grpc]\nendpoint = {endpoint:?}\n"
        ),
    )?;
    let _app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[
            ("OTEL_METRIC_EXPORT_INTERVAL", Some("200")),
            (
                codex_app_server_transport::REMOTE_CONTROL_DISABLED_ENV_VAR,
                Some("1"),
            ),
        ])
        .build_initialized_with_timeout(export_timeout)
        .await?;

    timeout(export_timeout, exports.wait_for(|count| *count > 0))
        .await
        .context("collector never received a SQLite log-write metric")??;
    timeout(
        export_timeout,
        ping_acknowledgements.wait_for(|count| *count > 0),
    )
    .await
    .context("collector response never produced an unsolicited HTTP/2 PING acknowledgment")??;
    let first_count = *exports.borrow_and_update();

    // Allow delayed startup batches, but require silence longer than one SQLite
    // flush interval plus one metric-export interval. Do not retain request bodies
    // or keep resetting the overall deadline when a broken exporter feeds itself.
    let quiescence = timeout(export_timeout, async {
        loop {
            match timeout(quiet_period, exports.changed()).await {
                Err(_) => return Ok::<(), watch::error::RecvError>(()),
                Ok(result) => result?,
            }
        }
    })
    .await;
    let final_count = *exports.borrow();
    quiescence.with_context(|| {
        format!(
            "OTLP gRPC status {grpc_status} did not become idle: SQLite log-write exports increased from {first_count} to {final_count}"
        )
    })??;

    Ok(())
}

#[tokio::test]
async fn app_server_emits_structured_tool_call_timing_event() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = create_mock_responses_server_sequence(vec![
        create_exec_command_sse_response("exec-call-1")?,
        create_final_assistant_message_sse_response("done")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .enable_feature(Feature::UnifiedExec)
        .with_root_config("compact_prompt = \"compact\"\nmodel_auto_compact_token_limit = 100000")
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_json_logging("warn,codex_core::tools::parallel=info")
        .build_initialized()
        .await?;

    let thread = app_server
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?
        .thread;

    let TurnStartResponse { turn } = app_server
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "run a command".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

    timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let mut tool_call = app_server
        .wait_for_json_log_event("codex.tool_call")
        .await?;
    let tool_call_object = tool_call
        .as_object_mut()
        .context("tool call log event must be an object")?;
    // JsonLogCapture already validates the timestamp as RFC 3339.
    tool_call_object
        .remove("timestamp")
        .context("tool call log event must include a timestamp")?;
    let fields = tool_call_object
        .get_mut("fields")
        .and_then(Value::as_object_mut)
        .context("tool call log event fields must be an object")?;
    let trace_id = fields
        .remove("trace_id")
        .context("tool call log event must include trace_id")?;
    anyhow::ensure!(trace_id.is_string(), "trace_id must be a string");
    let dispatch_duration_ms = fields
        .remove("dispatch_duration_ms")
        .and_then(|duration| duration.as_u64())
        .context("dispatch_duration_ms must be a nonnegative integer")?;
    let handler_duration_ms = fields
        .remove("handler_duration_ms")
        .and_then(|duration| duration.as_u64())
        .context("handler_duration_ms must be a nonnegative integer")?;
    let total_duration_ms = fields
        .remove("total_duration_ms")
        .and_then(|duration| duration.as_u64())
        .context("total_duration_ms must be a nonnegative integer")?;
    let accounted_duration_ms = dispatch_duration_ms
        .checked_add(handler_duration_ms)
        .context("dispatch and handler durations must not overflow")?;
    anyhow::ensure!(
        total_duration_ms >= accounted_duration_ms
            && total_duration_ms - accounted_duration_ms <= 1,
        "dispatch and handler durations must account for total duration within integer truncation"
    );

    assert_eq!(
        tool_call,
        json!({
            "level": "INFO",
            "fields": {
                "message": "tool call completed",
                "event.name": "codex.tool_call",
                "conversation.id": thread.id,
                "turn_id": turn.id,
                "tool_name": "exec_command",
                "call_id": "exec-call-1",
                "tool_source": "direct",
                "execution_started": true,
            },
            "target": "codex_core::tools::parallel",
        })
    );

    Ok(())
}
