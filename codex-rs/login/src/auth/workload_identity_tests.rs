use super::*;
use serde_json::json;

fn test_config(mapping_id: &str) -> WorkloadIdentityConfig {
    serde_json::from_value(json!({
        "identity_provider_id": "idp_test",
        "identity_provider_mapping_id": mapping_id,
        "audience": "api://codex-test",
        "token_url": "http://127.0.0.1:1/oauth/token",
        "credential_source": {
            "type": "file",
            "path": "/run/secrets/codex-wif/subject-token",
        },
    }))
    .expect("valid workload identity config")
}

#[test]
fn identical_config_reuses_process_scoped_external_auth() {
    let config = test_config("idpm_shared");
    let first = shared_workload_identity_external_auth(
        config.clone(),
        "client-test".to_string(),
        reqwest::Client::new(),
    );
    let second = shared_workload_identity_external_auth(
        config,
        "client-test".to_string(),
        reqwest::Client::new(),
    );

    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn different_config_uses_distinct_external_auth() {
    let first = shared_workload_identity_external_auth(
        test_config("idpm_first"),
        "client-test".to_string(),
        reqwest::Client::new(),
    );
    let second = shared_workload_identity_external_auth(
        test_config("idpm_second"),
        "client-test".to_string(),
        reqwest::Client::new(),
    );

    assert!(!Arc::ptr_eq(&first, &second));
}
