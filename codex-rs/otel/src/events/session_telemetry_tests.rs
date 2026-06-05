use super::SessionTelemetry;
use crate::metrics::tags::APP_VERSION_TAG;
use crate::sanitize_metric_tag_value;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use pretty_assertions::assert_eq;

#[test]
fn session_metric_tags_use_sanitized_app_version() {
    let mut telemetry = SessionTelemetry::new(
        ThreadId::new(),
        "model",
        "slug",
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "codex_desktop".to_string(),
        /*log_user_prompts*/ false,
        "unknown".to_string(),
        SessionSource::Cli,
    );
    telemetry.metadata.app_version = "0.136.0-alpha.1+frodex.1";
    telemetry.metric_app_version = sanitize_metric_tag_value(telemetry.metadata.app_version);

    let tags = telemetry.metadata_tag_refs().expect("metric tags");
    let app_version = tags
        .iter()
        .find_map(|(key, value)| (*key == APP_VERSION_TAG).then_some(*value));

    assert_eq!(app_version, Some("0.136.0-alpha.1_frodex.1"));
}
