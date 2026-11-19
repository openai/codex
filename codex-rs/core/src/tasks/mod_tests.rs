use super::SessionTask;
use super::SessionTaskResult;
use super::TASK_COMPACT_METRIC;
use super::emit_compact_metric;
use super::emit_turn_memory_metric;
use super::emit_turn_network_proxy_metric;
use crate::session::TurnInput;
use crate::session::session::Session;
use crate::session::tests::make_session_and_context_with_rx;
use crate::session::turn_context::TurnContext;
use crate::state::TaskKind;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_otel::SessionTelemetry;
use codex_otel::TURN_CRITICAL_PATH_CAPTURE_METRIC;
use codex_otel::TURN_CRITICAL_PATH_COMPONENT_METRIC;
use codex_otel::TURN_CRITICAL_PATH_DURATION_METRIC;
use codex_otel::TURN_MEMORY_METRIC;
use codex_otel::TURN_NETWORK_PROXY_METRIC;
use codex_otel::TURN_TOKEN_USAGE_METRIC;
use codex_otel::TURN_TOOL_CALL_METRIC;
use codex_otel::TURN_UNIFIED_EXEC_RUNNING_PROCESSES_METRIC;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::models::AudioReference;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::user_input::UserInput;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::Metric;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

struct PendingTask;

#[tokio::test]
async fn subagent_lifecycle_counts_each_regular_attempt_with_bounded_labels() {
    use crate::agent::turn_metrics;
    use codex_history::InitialHistory;
    use codex_history::ResumedHistory;
    use codex_history::RolloutItem;
    use codex_otel::MULTI_AGENT_TURN_LIFECYCLE_METRIC;
    use codex_protocol::models::ResponseInputItem;
    use codex_protocol::protocol::SubAgentSource;
    use codex_protocol::protocol::TurnAbortReason;

    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-core",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )
    .expect("metrics");
    let (mut session, _, _rx) = make_session_and_context_with_rx().await;
    let services = &mut Arc::get_mut(&mut session)
        .expect("unique test session")
        .services;
    services.session_telemetry = services
        .session_telemetry
        .clone()
        .with_product_sku(Some("codex"))
        .with_metrics(metrics.clone());
    let child = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id: ThreadId::new(),
        depth: 1,
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
    });
    let parent_input = ResponseInputItem::Message {
        role: "user".into(),
        content: vec![ContentItem::InputText {
            text: "a turn from the parent".into(),
        }],
        phase: None,
    };
    let inherited = vec![RolloutItem::ResponseItem(
        ResponseItem::from(parent_input).into(),
    )];
    let fork = InitialHistory::Forked(inherited.clone());
    assert!(crate::thread_rollout_truncation::initial_history_has_prior_user_turns(&fork));
    for source in [
        SessionSource::Cli,
        SessionSource::SubAgent(SubAgentSource::Review),
    ] {
        turn_metrics::register_thread(
            &session.services.thread_extension_data,
            &source,
            &InitialHistory::New,
        );
        let turn = session.new_default_turn().await;
        session
            .spawn_task(Arc::clone(&turn), Vec::new(), PendingTask)
            .await;
        session.on_task_finished(turn, Ok(None)).await;
    }
    turn_metrics::register_thread(&session.services.thread_extension_data, &child, &fork);
    let initial = session.new_default_turn().await;
    session
        .spawn_task(Arc::clone(&initial), Vec::new(), PendingTask)
        .await;
    session
        .on_task_finished(Arc::clone(&initial), Ok(None))
        .await;
    // A second observer or shutdown action must not report another outcome.
    turn_metrics::record_suspension(&initial);

    let follow_up = session.new_default_turn().await;
    session
        .spawn_task(Arc::clone(&follow_up), Vec::new(), PendingTask)
        .await;
    session
        .on_task_finished(follow_up, Err(CodexErr::ContextWindowExceeded))
        .await;

    turn_metrics::register_thread(
        &session.services.thread_extension_data,
        &child,
        &InitialHistory::Resumed(ResumedHistory {
            conversation_id: session.thread_id,
            history: Arc::new(inherited),
            history_revision: None,
            rollout_path: None,
        }),
    );
    let resumed = session.new_default_turn().await;
    session
        .spawn_task(Arc::clone(&resumed), Vec::new(), PendingTask)
        .await;
    session.abort_all_tasks(TurnAbortReason::Interrupted).await;
    let suspended = session.new_default_turn().await;
    session
        .spawn_task(Arc::clone(&suspended), Vec::new(), PendingTask)
        .await;
    turn_metrics::record_suspension(&suspended);
    session.abort_all_tasks(TurnAbortReason::Interrupted).await;

    let snapshot = metrics.snapshot().expect("snapshot metrics");
    let metric = find_metric(&snapshot, MULTI_AGENT_TURN_LIFECYCLE_METRIC);
    let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() else {
        panic!("expected a counter");
    };
    let actual: BTreeMap<_, _> = sum
        .data_points()
        .map(|point| {
            let mut tags = attributes_to_map(point.attributes());
            assert_eq!(tags.remove("product_sku").as_deref(), Some("codex"));
            assert_eq!(tags.remove("originator").as_deref(), Some("other"));
            let kind = tags.remove("turn_kind").expect("turn kind");
            let status = tags.remove("status").expect("turn status");
            match status.as_str() {
                "failed" => assert_eq!(
                    tags.remove("error_kind").as_deref(),
                    Some("context_window_exceeded")
                ),
                "interrupted" => {
                    assert_eq!(tags.remove("abort_reason").as_deref(), Some("interrupted"))
                }
                _ => {}
            }
            assert!(tags.is_empty(), "unexpected metric labels: {tags:?}");
            ((kind, status), point.value())
        })
        .collect();
    let expected = [
        ("initial", "started", 1),
        ("initial", "completed", 1),
        ("follow_up", "started", 1),
        ("follow_up", "failed", 1),
        ("resumed_thread", "started", 2),
        ("resumed_thread", "interrupted", 1),
        ("resumed_thread", "suspended", 1),
    ]
    .into_iter()
    .map(|(kind, status, count)| ((kind.to_owned(), status.to_owned()), count))
    .collect();
    assert_eq!(actual, expected);
}

impl SessionTask for PendingTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    fn span_name(&self) -> &'static str {
        "session_task.pending"
    }

    async fn run(
        self: Arc<Self>,
        _session: Arc<Session>,
        _turn_context: Arc<TurnContext>,
        _input: Vec<TurnInput>,
        _cancellation_token: CancellationToken,
    ) -> SessionTaskResult {
        std::future::pending().await
    }
}

#[tokio::test]
async fn task_finish_rejects_uploaded_audio_without_losing_other_pending_input() {
    let (session, turn_context, receiver) = make_session_and_context_with_rx().await;
    session
        .spawn_task(Arc::clone(&turn_context), Vec::new(), PendingTask)
        .await;
    while receiver.try_recv().is_ok() {}

    let turn_state = {
        let active_turn = session.active_turn.lock().await;
        Arc::clone(
            &active_turn
                .as_ref()
                .expect("active pending task")
                .turn_state,
        )
    };
    session
        .input_queue
        .extend_pending_input_for_turn_state(
            turn_state.as_ref(),
            vec![TurnInput::UserInput {
                content: vec![
                    UserInput::Text {
                        text: "preserve pending text".to_string(),
                        text_elements: Vec::new(),
                    },
                    UserInput::Audio {
                        audio: AudioReference::File {
                            file_id: "file_disabled_at_finish".to_string(),
                        },
                    },
                ]
                .into_iter()
                .map(codex_user_input::ResolvedUserInput::from_client)
                .collect(),
                client_id: None,
                metadata: Default::default(),
            }],
        )
        .await;

    session
        .on_task_finished(Arc::clone(&turn_context), /*task_result*/ Ok(None))
        .await;

    let history = session.clone_history().await;
    assert!(history.raw_items().any(|item| {
        matches!(item, ResponseItem::Message { content, .. } if content.iter().any(|part| {
            matches!(part, ContentItem::InputText { text } if text == "preserve pending text")
        }))
    }));
    assert!(history.raw_items().all(|item| {
        !matches!(item, ResponseItem::Message { content, .. } if content.iter().any(|part| {
            matches!(part, ContentItem::InputAudio {
                audio: AudioReference::File { file_id, .. },
            } if file_id == "file_disabled_at_finish")
        }))
    }));

    let (saw_error, completed) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        let mut saw_error = false;
        loop {
            match receiver.recv().await.expect("event channel open").msg {
                EventMsg::Error(error) => {
                    saw_error = true;
                    assert_eq!(error.codex_error_info, Some(CodexErrorInfo::BadRequest));
                }
                EventMsg::TurnComplete(completed) => break (saw_error, completed),
                _ => {}
            }
        }
    })
    .await
    .expect("task finish events");
    assert!(saw_error, "the rejected drain should emit an error event");
    assert_eq!(
        completed
            .error
            .as_ref()
            .and_then(|error| error.codex_error_info.clone()),
        Some(CodexErrorInfo::BadRequest)
    );
}

fn test_session_telemetry() -> SessionTelemetry {
    let exporter = InMemoryMetricExporter::default();
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory("test", "codex-core", env!("CARGO_PKG_VERSION"), exporter)
            .with_runtime_reader(),
    )
    .expect("in-memory metrics client");
    SessionTelemetry::new(
        ThreadId::new(),
        "gpt-5.4",
        "gpt-5.4",
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test_originator".to_string(),
        /*log_user_prompts*/ false,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics_without_metadata_tags(metrics)
}

fn find_metric<'a>(resource_metrics: &'a ResourceMetrics, name: &str) -> &'a Metric {
    for scope_metrics in resource_metrics.scope_metrics() {
        for metric in scope_metrics.metrics() {
            if metric.name() == name {
                return metric;
            }
        }
    }
    panic!("metric {name} missing");
}

fn attributes_to_map<'a>(
    attributes: impl Iterator<Item = &'a KeyValue>,
) -> BTreeMap<String, String> {
    attributes
        .map(|kv| (kv.key.as_str().to_string(), kv.value.as_str().to_string()))
        .collect()
}

fn metric_point(resource_metrics: &ResourceMetrics, name: &str) -> (BTreeMap<String, String>, u64) {
    let metric = find_metric(resource_metrics, name);
    match metric.data() {
        AggregatedMetrics::U64(data) => match data {
            MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                let point = points[0];
                (attributes_to_map(point.attributes()), point.value())
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    }
}

#[derive(Clone, Copy)]
enum UsageScenario {
    ResponseOnly,
    ResponseAfterStepSwitch,
    CompactThenResponse,
    CompactThenStop,
    NoResponse,
}

#[test_case::test_case(UsageScenario::ResponseOnly; "response_only")]
#[test_case::test_case(UsageScenario::ResponseAfterStepSwitch; "response_after_step_switch")]
#[test_case::test_case(UsageScenario::CompactThenResponse; "compact_then_response")]
#[test_case::test_case(UsageScenario::CompactThenStop; "compact_then_stop")]
#[test_case::test_case(UsageScenario::NoResponse; "no_response")]
#[tokio::test]
async fn turn_completion_metrics_follow_model_switch(scenario: UsageScenario) {
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-core",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )
    .expect("in-memory metrics client");
    let (mut session, mut turn_context, receiver) = make_session_and_context_with_rx().await;
    let session_telemetry = session
        .services
        .session_telemetry
        .clone()
        .with_metrics(metrics.clone());
    Arc::get_mut(&mut session)
        .expect("session should be uniquely owned")
        .services
        .session_telemetry = session_telemetry.clone();
    Arc::get_mut(&mut turn_context)
        .expect("turn context should be uniquely owned")
        .session_telemetry = session_telemetry;

    let next_model = if turn_context.model_info().slug == "gpt-5.4" {
        "gpt-5.2"
    } else {
        "gpt-5.4"
    };
    let previous_context = turn_context;
    let turn_context = Arc::new(
        previous_context
            .with_model(next_model.to_string(), &session.services.models_manager)
            .await,
    );
    // Earlier session usage must not leak into this turn's per-model samples.
    session
        .update_token_usage_info(
            &previous_context,
            TokenUsage {
                input_tokens: 1_000,
                total_tokens: 1_000,
                ..TokenUsage::default()
            }
            .into(),
            /*includes_reasoning*/ false,
        )
        .await
        .expect("earlier session usage should be recorded");
    session
        .spawn_task(Arc::clone(&turn_context), Vec::new(), PendingTask)
        .await;
    let mut expected_usage = Vec::new();
    if matches!(
        scenario,
        UsageScenario::CompactThenResponse | UsageScenario::CompactThenStop
    ) {
        let compaction_usage = TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 30,
            cache_write_input_tokens: 20,
            output_tokens: 70,
            reasoning_output_tokens: 40,
            total_tokens: 170,
            codex_rollout_budget_units: None,
        };
        // Local pre-turn compaction records usage with the previous model's context.
        session
            .update_token_usage_info(
                &previous_context,
                compaction_usage.clone().into(),
                /*includes_reasoning*/ false,
            )
            .await
            .expect("compaction usage should be recorded");
        expected_usage.push((previous_context.model_info().slug.clone(), compaction_usage));
    }
    if matches!(
        scenario,
        UsageScenario::ResponseOnly
            | UsageScenario::ResponseAfterStepSwitch
            | UsageScenario::CompactThenResponse
    ) {
        let response_context = if matches!(scenario, UsageScenario::ResponseAfterStepSwitch) {
            &previous_context
        } else {
            &turn_context
        };
        let response_usage = TokenUsage {
            input_tokens: 10,
            cached_input_tokens: 3,
            cache_write_input_tokens: 2,
            output_tokens: 7,
            reasoning_output_tokens: 4,
            total_tokens: 17,
            codex_rollout_budget_units: None,
        };
        // Multiple requests for one model must still produce one turn histogram sample.
        for _ in 0..2 {
            session
                .record_token_usage_info(
                    &turn_context,
                    &response_context.initial_settings,
                    response_usage.clone().into(),
                    crate::session::TokenUsageContext::Sampling {
                        includes_reasoning: false,
                        input_item_count: 0,
                    },
                )
                .await
                .expect("response usage should be recorded");
        }
        let mut total_usage = response_usage.clone();
        total_usage.add_assign(&response_usage);
        expected_usage.push((response_context.model_info().slug.clone(), total_usage));
    }
    if matches!(scenario, UsageScenario::NoResponse) {
        expected_usage.push((next_model.to_string(), TokenUsage::default()));
    }

    let task_result = if matches!(scenario, UsageScenario::CompactThenStop) {
        // A PostCompact hook can stop the turn before the new model samples.
        Err(CodexErr::TurnAborted)
    } else {
        Ok(None)
    };
    let expected_root = turn_context.root_turn_id();
    session.on_task_finished(turn_context, task_result).await;
    let mut terminal_root = None;
    while let Ok(event) = receiver.try_recv() {
        match event.msg {
            codex_protocol::protocol::EventMsg::TurnComplete(event) => {
                terminal_root = event.root_turn_id;
            }
            codex_protocol::protocol::EventMsg::TurnAborted(event) => {
                terminal_root = event.root_turn_id;
            }
            _ => {}
        }
    }
    assert_eq!(terminal_root, Some(expected_root));

    let snapshot = metrics.snapshot().expect("runtime metrics snapshot");
    let token_usage_metric = find_metric(&snapshot, TURN_TOKEN_USAGE_METRIC);
    let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = token_usage_metric.data() else {
        panic!("expected token usage histogram");
    };
    let mut token_usage_points = histogram
        .data_points()
        .map(|point| {
            let mut attributes = attributes_to_map(point.attributes());
            (
                (
                    attributes
                        .remove("token_type")
                        .expect("token usage metric should include a token type"),
                    attributes
                        .remove("model")
                        .expect("token usage metric should include a model"),
                ),
                point.count(),
                point.sum(),
            )
        })
        .collect::<Vec<_>>();
    let mut expected_points = expected_usage
        .into_iter()
        .flat_map(|(model, usage)| {
            [
                ("cache_write_input", usage.cache_write_input_tokens),
                ("cached_input", usage.cached_input()),
                ("input", usage.input_tokens),
                ("output", usage.output_tokens),
                ("reasoning_output", usage.reasoning_output_tokens),
                ("total", usage.total_tokens),
            ]
            .map(|(token_type, value)| ((token_type.to_string(), model.clone()), 1, value as f64))
        })
        .collect::<Vec<_>>();
    token_usage_points.sort_by(|left, right| left.0.cmp(&right.0));
    expected_points.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(token_usage_points, expected_points);

    let tool_call_metric = find_metric(&snapshot, TURN_TOOL_CALL_METRIC);
    let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = tool_call_metric.data() else {
        panic!("expected tool call histogram");
    };
    let tool_call_models = histogram
        .data_points()
        .map(|point| {
            attributes_to_map(point.attributes())
                .remove("model")
                .expect("tool call metric should include a model")
        })
        .collect::<Vec<_>>();
    assert_eq!(tool_call_models, vec![next_model.to_string()]);

    let counter_models = [
        TURN_MEMORY_METRIC,
        TURN_NETWORK_PROXY_METRIC,
        TURN_UNIFIED_EXEC_RUNNING_PROCESSES_METRIC,
    ]
    .into_iter()
    .map(|name| {
        let (mut attributes, _value) = metric_point(&snapshot, name);
        (
            name.to_string(),
            attributes
                .remove("model")
                .expect("turn counter should include a model"),
        )
    })
    .collect::<BTreeMap<_, _>>();
    assert_eq!(
        counter_models,
        BTreeMap::from([
            (TURN_MEMORY_METRIC.to_string(), next_model.to_string()),
            (
                TURN_NETWORK_PROXY_METRIC.to_string(),
                next_model.to_string(),
            ),
            (
                TURN_UNIFIED_EXEC_RUNNING_PROCESSES_METRIC.to_string(),
                next_model.to_string(),
            ),
        ])
    );
}

#[test]
fn emit_turn_network_proxy_metric_records_active_turn() {
    let session_telemetry = test_session_telemetry();

    emit_turn_network_proxy_metric(
        &session_telemetry,
        /*network_proxy_active*/ true,
        ("tmp_mem_enabled", "true"),
    );

    let snapshot = session_telemetry
        .snapshot_metrics()
        .expect("runtime metrics snapshot");
    let (attrs, value) = metric_point(&snapshot, TURN_NETWORK_PROXY_METRIC);

    assert_eq!(value, 1);
    assert_eq!(
        attrs,
        BTreeMap::from([
            ("active".to_string(), "true".to_string()),
            ("tmp_mem_enabled".to_string(), "true".to_string()),
        ])
    );
}

#[test]
fn emit_turn_network_proxy_metric_records_inactive_turn() {
    let session_telemetry = test_session_telemetry();

    emit_turn_network_proxy_metric(
        &session_telemetry,
        /*network_proxy_active*/ false,
        ("tmp_mem_enabled", "false"),
    );

    let snapshot = session_telemetry
        .snapshot_metrics()
        .expect("runtime metrics snapshot");
    let (attrs, value) = metric_point(&snapshot, TURN_NETWORK_PROXY_METRIC);

    assert_eq!(value, 1);
    assert_eq!(
        attrs,
        BTreeMap::from([
            ("active".to_string(), "false".to_string()),
            ("tmp_mem_enabled".to_string(), "false".to_string()),
        ])
    );
}

#[test]
fn emit_turn_memory_metric_records_read_allowed_with_citations() {
    let session_telemetry = test_session_telemetry();

    emit_turn_memory_metric(
        &session_telemetry,
        /*feature_enabled*/ true,
        /*config_enabled*/ true,
        /*has_citations*/ true,
    );

    let snapshot = session_telemetry
        .snapshot_metrics()
        .expect("runtime metrics snapshot");
    let (attrs, value) = metric_point(&snapshot, TURN_MEMORY_METRIC);

    assert_eq!(value, 1);
    assert_eq!(
        attrs,
        BTreeMap::from([
            ("config_use_memories".to_string(), "true".to_string()),
            ("feature_enabled".to_string(), "true".to_string()),
            ("has_citations".to_string(), "true".to_string()),
            ("read_allowed".to_string(), "true".to_string()),
        ])
    );
}

#[test]
fn emit_turn_memory_metric_records_config_disabled_without_citations() {
    let session_telemetry = test_session_telemetry();

    emit_turn_memory_metric(
        &session_telemetry,
        /*feature_enabled*/ true,
        /*config_enabled*/ false,
        /*has_citations*/ false,
    );

    let snapshot = session_telemetry
        .snapshot_metrics()
        .expect("runtime metrics snapshot");
    let (attrs, value) = metric_point(&snapshot, TURN_MEMORY_METRIC);

    assert_eq!(value, 1);
    assert_eq!(
        attrs,
        BTreeMap::from([
            ("config_use_memories".to_string(), "false".to_string()),
            ("feature_enabled".to_string(), "true".to_string()),
            ("has_citations".to_string(), "false".to_string()),
            ("read_allowed".to_string(), "false".to_string()),
        ])
    );
}

#[test]
fn emit_compact_metric_records_manual_remote_v2() {
    let session_telemetry = test_session_telemetry();

    emit_compact_metric(&session_telemetry, "remote_v2", /*manual*/ true);

    let snapshot = session_telemetry
        .snapshot_metrics()
        .expect("runtime metrics snapshot");
    let (attrs, value) = metric_point(&snapshot, TASK_COMPACT_METRIC);

    assert_eq!(value, 1);
    assert_eq!(
        attrs,
        BTreeMap::from([
            ("manual".to_string(), "true".to_string()),
            ("type".to_string(), "remote_v2".to_string()),
        ])
    );
}

#[test]
fn emit_compact_metric_records_auto_local() {
    let session_telemetry = test_session_telemetry();

    emit_compact_metric(&session_telemetry, "local", /*manual*/ false);

    let snapshot = session_telemetry
        .snapshot_metrics()
        .expect("runtime metrics snapshot");
    let (attrs, value) = metric_point(&snapshot, TASK_COMPACT_METRIC);

    assert_eq!(value, 1);
    assert_eq!(
        attrs,
        BTreeMap::from([
            ("manual".to_string(), "false".to_string()),
            ("type".to_string(), "local".to_string()),
        ])
    );
}

struct UncooperativeTimedTask {
    entered: Arc<tokio::sync::Notify>,
    finish: Arc<tokio::sync::Notify>,
}

impl SessionTask for UncooperativeTimedTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }
    fn span_name(&self) -> &'static str {
        "session_task.uncooperative_timing"
    }
    async fn run(
        self: Arc<Self>,
        _session: Arc<Session>,
        context: Arc<TurnContext>,
        _input: Vec<TurnInput>,
        _cancellation_token: CancellationToken,
    ) -> SessionTaskResult {
        let timing = &context.turn_timing_state;
        timing.enable_critical_path();
        let batch = codex_direct_tool_timing::ToolBatchTiming::default();
        let now = std::time::Instant::now();
        let _blocking;
        let _work = timing.direct_tool_timings.start_turn_direct_call(
            "uncooperative",
            &context.sub_id,
            batch.clone(),
            now,
            timing.snapshot_path(now),
        );
        _blocking = timing.begin_tool_blocking(Some(batch));
        let _review = _work.start_review();
        self.entered.notify_one();
        self.finish.notified().await;
        Ok(None)
    }
}

#[test_case::test_case(false; "forced teardown")]
#[test_case::test_case(true; "finished before wait")]
#[tokio::test(flavor = "current_thread")]
async fn abort_drops_timing_guards_before_profile_completion(finished_before_wait: bool) {
    use codex_analytics::IncompleteReason::ForcedTeardown;
    use codex_analytics::TurnCriticalPath;
    use codex_protocol::protocol::TurnAbortReason;

    let (session, context, _receiver) = make_session_and_context_with_rx().await;
    let entered = Arc::new(tokio::sync::Notify::new());
    let finish = Arc::new(tokio::sync::Notify::new());
    session
        .spawn_task(
            Arc::clone(&context),
            Vec::new(),
            UncooperativeTimedTask {
                entered: Arc::clone(&entered),
                finish: Arc::clone(&finish),
            },
        )
        .await;
    entered.notified().await;
    let turn = session
        .active_turn
        .lock()
        .await
        .take()
        .expect("active turn");
    if finished_before_wait {
        // The task exits after the abort claims the turn but before it waits.
        let done = turn.task.as_ref().expect("running task").done.notified();
        finish.notify_one();
        done.await;
    }
    session
        .abort_taken_tasks(Some(turn), TurnAbortReason::Interrupted)
        .await;

    let timing = &context.turn_timing_state;
    assert!(
        timing
            .direct_tool_timings
            .owner_for_turn("uncooperative", &context.sub_id)
            .is_none()
    );
    let (_, _, profile) = timing.complete_profile_and_duration_ms().await;
    if finished_before_wait {
        assert!(matches!(
            profile.critical_path,
            TurnCriticalPath::Complete(_)
        ));
    } else {
        assert_eq!(
            profile.critical_path,
            TurnCriticalPath::Incomplete(vec![ForcedTeardown])
        );
    }
}

#[tokio::test]
async fn turn_critical_path_metrics_count_each_capture_once() {
    use codex_analytics::IncompleteReason::Unclassified;
    use codex_analytics::TurnCriticalPath;
    use codex_analytics::TurnCriticalPathDurations;

    let (session, mut context, _rx) = make_session_and_context_with_rx().await;
    let context = Arc::get_mut(&mut context).unwrap();
    context.session_telemetry = test_session_telemetry();
    let complete = TurnCriticalPath::Complete(TurnCriticalPathDurations {
        duration_ms: 60,
        inference_ms: 31,
        sampling_ms: 31,
        compaction_ms: 0,
        auto_review_ms: 2,
        auto_review_overhead_ms: None,
        tools_ms: 10,
        user_wait_ms: 0,
        overhead_ms: 14,
        unknown_ms: 3,
    });
    let _interest_cache_guard =
        tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let incomplete = TurnCriticalPath::Incomplete(vec![Unclassified]);
    for (sku, path) in [
        (Some("codex"), complete),
        (Some("codex"), TurnCriticalPath::Disabled),
        (None, incomplete.clone()),
        (Some("unrecognized-sku"), incomplete),
    ] {
        Arc::make_mut(&mut context.config).apps_mcp_product_sku = sku.map(str::to_owned);
        context.turn_timing_state = Arc::default();
        let (_, _, mut profile) = context
            .turn_timing_state
            .complete_profile_and_duration_ms()
            .await;
        profile.critical_path = path;
        let buffer: &std::sync::Mutex<Vec<u8>> =
            Box::leak(Box::new(std::sync::Mutex::new(Vec::new())));
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(tracing_test::internal::MockWriter::new(buffer))
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            for _ in 0..2 {
                crate::turn_timing::record_terminal_turn_profile(
                    session.thread_id,
                    context,
                    &profile,
                );
            }
        });
        let trace = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        match &profile.critical_path {
            TurnCriticalPath::Complete(_) => {
                assert!(trace.contains("critical_path_status=\"complete\""));
                assert!(trace.contains("critical_path_inference_ms=31"));
                assert!(trace.contains("critical_path_user_wait_ms=0"));
            }
            TurnCriticalPath::Disabled | TurnCriticalPath::Incomplete(_) => {
                assert!(!trace.contains("critical_path_duration_ms="));
                let status = if matches!(profile.critical_path, TurnCriticalPath::Disabled) {
                    "disabled"
                } else {
                    "incomplete"
                };
                assert!(trace.contains(&format!("critical_path_status=\"{status}\"")));
            }
        }
    }

    let snapshot = context.session_telemetry.snapshot_metrics().unwrap();
    let AggregatedMetrics::U64(MetricData::Sum(captures)) =
        find_metric(&snapshot, TURN_CRITICAL_PATH_CAPTURE_METRIC).data()
    else {
        panic!("expected capture counter");
    };
    let counts = captures
        .data_points()
        .map(|point| {
            let tags = attributes_to_map(point.attributes());
            (
                (tags["product_sku"].clone(), tags["capture_status"].clone()),
                point.value(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        counts,
        BTreeMap::from([
            (("codex".into(), "complete".into()), 1),
            (("unknown".into(), "incomplete".into()), 1),
            (("other".into(), "incomplete".into()), 1),
        ])
    );

    for (name, expected) in [
        (
            TURN_CRITICAL_PATH_DURATION_METRIC,
            BTreeMap::from([("total", 60.0)]),
        ),
        (
            TURN_CRITICAL_PATH_COMPONENT_METRIC,
            BTreeMap::from([
                ("inference", 31.0),
                ("auto_review", 2.0),
                ("tools", 10.0),
                ("user_wait", 0.0),
                ("overhead", 14.0),
                ("unknown", 3.0),
            ]),
        ),
    ] {
        let AggregatedMetrics::F64(MetricData::Histogram(histogram)) =
            find_metric(&snapshot, name).data()
        else {
            panic!("expected duration histogram");
        };
        assert_eq!(histogram.data_points().count(), expected.len());
        for point in histogram.data_points() {
            let tags = attributes_to_map(point.attributes());
            assert_eq!(tags["product_sku"], "codex");
            assert_eq!(
                point
                    .bounds()
                    .filter(|bound| *bound > 120_000.0)
                    .collect::<Vec<_>>(),
                vec![
                    180_000.0,
                    300_000.0,
                    600_000.0,
                    900_000.0,
                    1_800_000.0,
                    3_600_000.0,
                    7_200_000.0
                ]
            );
            let component = tags.get("component").map_or("total", String::as_str);
            assert_eq!((point.count(), point.sum()), (1, expected[component]));
        }
    }
}
