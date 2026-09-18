//! External-agent configuration import event tests.

use crate::events::CodexOnboardingExternalAgentImportFailureEventRequest;
use crate::events::CodexOnboardingExternalAgentImportFailureMetadata;
use crate::events::TrackEventRequest;
use crate::facts::AnalyticsFact;
use crate::facts::CustomAnalyticsFact;
use crate::facts::ExternalAgentConfigImportCompletedInput;
use crate::facts::ExternalAgentConfigImportFailureInput;
use crate::reducer::AnalyticsReducer;
use codex_login::default_client::originator;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn reducer_ingests_external_agent_config_import_completed_fact() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::ExternalAgentConfigImportCompleted(
                ExternalAgentConfigImportCompletedInput {
                    import_id: "import-1".to_string(),
                    source: "app_server".to_string(),
                    provider_id: "test-provider-42".to_string(),
                    item_type: "PLUGINS".to_string(),
                    success_count: 2,
                    failed_count: 1,
                },
            )),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(
        payload,
        json!([{
            "event_type": "codex_onboarding_external_agent_import_complete",
            "event_params": {
                "import_id": "import-1",
                "source": "app_server",
                "provider_id": "test-provider-42",
                "type": "PLUGINS",
                "success_count": 2,
                "failed_count": 1,
                "product_client_id": originator().value,
            }
        }])
    );
}

#[test]
fn external_agent_config_import_failure_event_serializes_expected_shape() {
    let event = TrackEventRequest::ExternalAgentConfigImportFailure(
        CodexOnboardingExternalAgentImportFailureEventRequest {
            event_type: "codex_onboarding_external_agent_import_failure",
            event_params: CodexOnboardingExternalAgentImportFailureMetadata {
                import_id: "import-1".to_string(),
                source: "app_server".to_string(),
                provider_id: "test-provider-42".to_string(),
                item_type: "PLUGINS".to_string(),
                failure_stage: "plugin_import".to_string(),
                error_type: "plugin_import".to_string(),
                sub_error_type: Some("failed_to_copy_plugin_file".to_string()),
                product_client_id: Some(originator().value),
            },
        },
    );

    let payload = serde_json::to_value(&event).expect("serialize import failure event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_onboarding_external_agent_import_failure",
            "event_params": {
                "import_id": "import-1",
                "source": "app_server",
                "provider_id": "test-provider-42",
                "type": "PLUGINS",
                "failure_stage": "plugin_import",
                "error_type": "plugin_import",
                "sub_error_type": "failed_to_copy_plugin_file",
                "product_client_id": originator().value,
            }
        })
    );
}

#[tokio::test]
async fn reducer_ingests_external_agent_config_import_failure_fact() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::ExternalAgentConfigImportFailure(
                ExternalAgentConfigImportFailureInput {
                    import_id: "import-1".to_string(),
                    source: "app_server".to_string(),
                    provider_id: "test-provider-42".to_string(),
                    item_type: "PLUGINS".to_string(),
                    failure_stage: "plugin_import".to_string(),
                    error_type: "plugin_import".to_string(),
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
            "event_type": "codex_onboarding_external_agent_import_failure",
            "event_params": {
                "import_id": "import-1",
                "source": "app_server",
                "provider_id": "test-provider-42",
                "type": "PLUGINS",
                "failure_stage": "plugin_import",
                "error_type": "plugin_import",
                "sub_error_type": "failed_to_copy_plugin_file",
                "product_client_id": originator().value,
            }
        }])
    );
}
