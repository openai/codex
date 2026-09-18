//! Plugin lifecycle, usage, measurements, and event deduplication tests.

use crate::client::AnalyticsEventsQueue;
use crate::events::CodexPluginEventRequest;
use crate::events::CodexPluginInstallFailedEventRequest;
use crate::events::CodexPluginInstallFailedMetadata;
use crate::events::CodexPluginUsedEventRequest;
use crate::events::TrackEventRequest;
use crate::events::codex_plugin_metadata;
use crate::events::codex_plugin_used_metadata;
use crate::facts::AnalyticsFact;
use crate::facts::CustomAnalyticsFact;
use crate::facts::PluginInstallFailedInput;
use crate::facts::PluginInstallRequestSource;
use crate::facts::PluginInstallRequested;
use crate::facts::PluginInstallRequestedInput;
use crate::facts::PluginInstallRequestedPlugin;
use crate::facts::PluginInstallSource;
use crate::facts::PluginMeasurementRow;
use crate::facts::PluginMeasurementsInput;
use crate::facts::PluginState;
use crate::facts::PluginStateChangedInput;
use crate::facts::PluginUsedInput;
use crate::reducer::AnalyticsReducer;
use crate::tests::support::TEST_PRODUCT_CLIENT_ID;
use crate::tests::support::sample_plugin_metadata;
use crate::tests::support::test_tracking_context;
use codex_login::default_client::originator;
use codex_plugin::PluginTelemetryMetadata;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;

fn plugin_measurements(rows: Vec<PluginMeasurementRow>) -> PluginMeasurementsInput {
    PluginMeasurementsInput {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        item_id: "item-1".to_string(),
        originator: "codex_cli_rs".to_string(),
        model_slug: None,
        reasoning_effort: None,
        plugin_id: "sample@openai-curated".to_string(),
        execution_id: "execution-1".to_string(),
        operation: "security_scan".to_string(),
        rows,
    }
}

#[tokio::test]
async fn plugin_measurement_batch_emits_directly_and_filters_invalid_rows() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    let mut too_many_dimensions = BTreeMap::new();
    for index in 0..9 {
        too_many_dimensions.insert(format!("dimension_{index}"), "allowed".to_string());
    }
    let mut measurements = plugin_measurements(vec![
        PluginMeasurementRow {
            measurement_name: "finding_count".to_string(),
            number_value: 3.0,
            dimensions: BTreeMap::from([("severity".to_string(), "high".to_string())]),
        },
        PluginMeasurementRow {
            measurement_name: "non_finite".to_string(),
            number_value: f64::NAN,
            dimensions: BTreeMap::new(),
        },
        PluginMeasurementRow {
            measurement_name: "too_many_dimensions".to_string(),
            number_value: 1.0,
            dimensions: too_many_dimensions,
        },
        PluginMeasurementRow {
            measurement_name: "files_scanned".to_string(),
            number_value: 17.0,
            dimensions: BTreeMap::new(),
        },
    ]);
    measurements.model_slug = Some("invoking-model".to_string());
    measurements.reasoning_effort = Some("max".to_string());
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::PluginMeasurements(measurements)),
            &mut events,
        )
        .await;
    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(
        payload,
        json!([
            {
                "event_type": "codex_plugin_measurement_event",
                "event_params": {
                    "thread_id": "thread-1",
                    "turn_id": "turn-1",
                    "item_id": "item-1",
                    "plugin_id": "sample@openai-curated",
                    "execution_id": "execution-1",
                    "operation": "security_scan",
                    "measurement_name": "finding_count",
                    "originator": "codex_cli_rs",
                    "model_slug": "invoking-model",
                    "reasoning_effort": "max",
                    "number_value": 3.0,
                    "dimensions": {"severity": "high"},
                },
            },
            {
                "event_type": "codex_plugin_measurement_event",
                "event_params": {
                    "thread_id": "thread-1",
                    "turn_id": "turn-1",
                    "item_id": "item-1",
                    "plugin_id": "sample@openai-curated",
                    "execution_id": "execution-1",
                    "operation": "security_scan",
                    "measurement_name": "files_scanned",
                    "originator": "codex_cli_rs",
                    "model_slug": "invoking-model",
                    "reasoning_effort": "max",
                    "number_value": 17.0,
                    "dimensions": null,
                },
            },
        ])
    );
}

#[test]
fn plugin_used_event_serializes_expected_shape() {
    let tracking = test_tracking_context("thread-3", "turn-3");
    let event = TrackEventRequest::PluginUsed(CodexPluginUsedEventRequest {
        event_type: "codex_plugin_used",
        event_params: codex_plugin_used_metadata(&tracking, sample_plugin_metadata()),
    });

    let payload = serde_json::to_value(&event).expect("serialize plugin used event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_plugin_used",
            "event_params": {
                "plugin_id": "sample@test",
                "remote_plugin_id": null,
                "plugin_name": "sample",
                "marketplace_name": "test",
                "has_skills": true,
                "mcp_server_count": 2,
                "connector_ids": ["calendar", "drive"],
                "product_client_id": TEST_PRODUCT_CLIENT_ID,
                "mcp_server_names": ["mcp-1", "mcp-2"],
                "thread_id": "thread-3",
                "turn_id": "turn-3",
                "model_slug": "gpt-5"
            }
        })
    );
}

#[tokio::test]
async fn reducer_ingests_plugin_used_fact() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    let tracking = test_tracking_context("thread-1", "turn-1");

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::PluginUsed(PluginUsedInput {
                tracking,
                plugin: sample_plugin_metadata(),
            })),
            &mut events,
        )
        .await;

    let [TrackEventRequest::PluginUsed(event)] = events.as_slice() else {
        panic!("expected a plugin usage event");
    };
    assert_eq!(
        (
            event.event_type,
            event.event_params.plugin.product_client_id.as_deref()
        ),
        ("codex_plugin_used", Some(TEST_PRODUCT_CLIENT_ID))
    );
}

#[test]
fn plugin_management_event_serializes_expected_shape() {
    let event = TrackEventRequest::PluginInstalled(CodexPluginEventRequest {
        event_type: "codex_plugin_installed",
        event_params: codex_plugin_metadata(sample_plugin_metadata()),
    });

    let payload = serde_json::to_value(&event).expect("serialize plugin installed event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_plugin_installed",
            "event_params": {
                "plugin_id": "sample@test",
                "remote_plugin_id": null,
                "plugin_name": "sample",
                "marketplace_name": "test",
                "has_skills": true,
                "mcp_server_count": 2,
                "connector_ids": ["calendar", "drive"],
                "product_client_id": originator().value
            }
        })
    );
}

#[test]
fn plugin_install_failed_event_serializes_expected_shape() {
    let event = TrackEventRequest::PluginInstallFailed(CodexPluginInstallFailedEventRequest {
        event_type: "codex_plugin_install_failed",
        event_params: CodexPluginInstallFailedMetadata {
            plugin: codex_plugin_metadata(sample_plugin_metadata()),
            source: PluginInstallSource::Manual,
            error_type: "store_io".to_string(),
            sub_error_type: Some("failed_to_copy_plugin_file".to_string()),
        },
    });

    let payload = serde_json::to_value(&event).expect("serialize plugin install failed event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_plugin_install_failed",
            "event_params": {
                "plugin_id": "sample@test",
                "remote_plugin_id": null,
                "plugin_name": "sample",
                "marketplace_name": "test",
                "has_skills": true,
                "mcp_server_count": 2,
                "connector_ids": ["calendar", "drive"],
                "product_client_id": originator().value,
                "source": "manual",
                "error_type": "store_io",
                "sub_error_type": "failed_to_copy_plugin_file"
            }
        })
    );
}

#[test]
fn plugin_management_event_keeps_plugin_id_local_when_remote_id_exists() {
    let mut plugin = sample_plugin_metadata();
    plugin.remote_plugin_id = Some("plugins~Plugin_remote".to_string());
    let event = TrackEventRequest::PluginInstalled(CodexPluginEventRequest {
        event_type: "codex_plugin_installed",
        event_params: codex_plugin_metadata(plugin),
    });

    let payload = serde_json::to_value(&event).expect("serialize plugin installed event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_plugin_installed",
            "event_params": {
                "plugin_id": "sample@test",
                "remote_plugin_id": "plugins~Plugin_remote",
                "plugin_name": "sample",
                "marketplace_name": "test",
                "has_skills": true,
                "mcp_server_count": 2,
                "connector_ids": ["calendar", "drive"],
                "product_client_id": originator().value
            }
        })
    );
}

#[test]
fn plugin_used_dedupe_is_keyed_by_turn_and_plugin() {
    let (sender, _receiver) = mpsc::channel(1);
    let queue = AnalyticsEventsQueue {
        sender,
        app_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
        plugin_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
    };
    let plugin = sample_plugin_metadata();

    let turn_1 = test_tracking_context("thread-1", "turn-1");
    let turn_2 = test_tracking_context("thread-1", "turn-2");

    assert_eq!(queue.should_enqueue_plugin_used(&turn_1, &plugin), true);
    assert_eq!(queue.should_enqueue_plugin_used(&turn_1, &plugin), false);
    assert_eq!(queue.should_enqueue_plugin_used(&turn_2, &plugin), true);
}

#[tokio::test]
async fn reducer_ingests_plugin_state_changed_fact() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::PluginStateChanged(
                PluginStateChangedInput {
                    plugin: sample_plugin_metadata(),
                    state: PluginState::Disabled,
                },
            )),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(
        payload,
        json!([{
            "event_type": "codex_plugin_disabled",
            "event_params": {
                "plugin_id": "sample@test",
                "remote_plugin_id": null,
                "plugin_name": "sample",
                "marketplace_name": "test",
                "has_skills": true,
                "mcp_server_count": 2,
                "connector_ids": ["calendar", "drive"],
                "product_client_id": originator().value
            }
        }])
    );
}

#[tokio::test]
async fn reducer_ingests_plugin_install_requested_fact() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    let tracking = test_tracking_context("thread-1", "turn-1");
    let request = PluginInstallRequested {
        suggestion_id: "request_plugin_install_call-1".to_string(),
        plugins: vec![
            PluginInstallRequestedPlugin {
                plugin_id: "calendar@openai-curated-remote".to_string(),
                remote_plugin_id: Some("plugin_calendar".to_string()),
                plugin_name: "Calendar".to_string(),
                connector_ids: vec!["connector_calendar".to_string()],
            },
            PluginInstallRequestedPlugin {
                plugin_id: "github@openai-curated-remote".to_string(),
                remote_plugin_id: None,
                plugin_name: "GitHub".to_string(),
                connector_ids: vec!["connector_github".to_string()],
            },
        ],
        source: PluginInstallRequestSource::EndpointRecommendation,
    };

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::PluginInstallRequested(
                PluginInstallRequestedInput { tracking, request },
            )),
            &mut events,
        )
        .await;

    assert_eq!(
        serde_json::to_value(&events).expect("serialize events"),
        json!([{
            "event_type": "codex_plugin_install_requested",
            "event_params": {
                "suggestion_id": "request_plugin_install_call-1",
                "plugins": [{
                    "plugin_id": "calendar@openai-curated-remote",
                    "remote_plugin_id": "plugin_calendar",
                    "plugin_name": "Calendar",
                    "connector_ids": ["connector_calendar"],
                }, {
                    "plugin_id": "github@openai-curated-remote",
                    "remote_plugin_id": null,
                    "plugin_name": "GitHub",
                    "connector_ids": ["connector_github"],
                }],
                "source": "endpoint_recommendation",
                "thread_id": "thread-1",
                "turn_id": "turn-1",
                "model_slug": "gpt-5",
                "product_client_id": originator().value,
            }
        }])
    );
}

#[tokio::test]
async fn reducer_ingests_plugin_install_failed_fact() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::PluginInstallFailed(
                PluginInstallFailedInput {
                    plugin: sample_plugin_metadata(),
                    source: PluginInstallSource::ExternalAgentMigration,
                    error_type: "invalid_plugin".to_string(),
                    sub_error_type: Some("failed_to_copy_plugin_file".to_string()),
                },
            )),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(
        payload,
        json!([{
            "event_type": "codex_plugin_install_failed",
            "event_params": {
                "plugin_id": "sample@test",
                "remote_plugin_id": null,
                "plugin_name": "sample",
                "marketplace_name": "test",
                "has_skills": true,
                "mcp_server_count": 2,
                "connector_ids": ["calendar", "drive"],
                "product_client_id": originator().value,
                "source": "external_agent_migration",
                "error_type": "invalid_plugin",
                "sub_error_type": "failed_to_copy_plugin_file"
            }
        }])
    );
}

#[tokio::test]
async fn reducer_ingests_plugin_install_failed_fact_without_detail() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    let plugin = PluginTelemetryMetadata {
        plugin_id: None,
        remote_plugin_id: Some("plugins~Plugin_00000000000000000000000000000000".to_string()),
        capability_summary: None,
    };

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::PluginInstallFailed(
                PluginInstallFailedInput {
                    plugin,
                    source: PluginInstallSource::Manual,
                    error_type: "remote_catalog_unexpected_status".to_string(),
                    sub_error_type: None,
                },
            )),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(
        payload,
        json!([{
            "event_type": "codex_plugin_install_failed",
            "event_params": {
                "plugin_id": null,
                "remote_plugin_id": "plugins~Plugin_00000000000000000000000000000000",
                "plugin_name": null,
                "marketplace_name": null,
                "has_skills": null,
                "mcp_server_count": null,
                "connector_ids": null,
                "product_client_id": originator().value,
                "source": "manual",
                "error_type": "remote_catalog_unexpected_status",
                "sub_error_type": null
            }
        }])
    );
}
