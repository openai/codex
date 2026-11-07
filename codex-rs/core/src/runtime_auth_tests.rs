//! Verifies the supported destinations and environment boundaries for Realtime auth.

#![cfg(test)]

use super::*;
use codex_model_provider::validate_runtime_environment;
use pretty_assertions::assert_eq;

#[test]
fn realtime_auth_accepts_only_secure_first_party_destinations() {
    for url in [
        "https://api.openai.com/v1",
        "https://api.openai.org/v1",
        "wss://api.openai.com/v1",
        "wss://chatgpt.com/backend-api",
    ] {
        assert!(is_openai_realtime_url(url).unwrap(), "{url}");
    }
    for url in [
        "ws://api.openai.com/v1",
        "http://api.openai.com/v1",
        "wss://unknown.openai.com/v1",
        "wss://api.openai.com:8443/v1",
        "wss://user:secret@api.openai.com/v1",
    ] {
        assert!(is_openai_realtime_url(url).is_err(), "{url}");
    }
    assert!(!is_openai_realtime_url("wss://voice.example.com").unwrap());
}

#[test]
fn realtime_rejects_a_different_registration_environment() {
    for (chatgpt_base_url, expected_staging) in [
        ("https://chatgpt.com/backend-api", false),
        ("https://chatgpt-staging.com/backend-api", true),
    ] {
        let runtime = RuntimeAuthManager::for_chatgpt(
            AuthManager::from_auth_for_testing(codex_login::CodexAuth::from_api_key("test-key")),
            uuid::Uuid::nil().to_string(),
            chatgpt_base_url.to_owned(),
            RuntimeRegistrationMetadata {
                harness: None,
                harness_version: None,
                role: RuntimeRole::Orchestrator,
            },
        );
        for (destination, staging) in [
            ("wss://api.openai.com/v1", false),
            ("https://chatgpt.com/backend-api", false),
            ("wss://api.openai-staging.com/v1", true),
            ("https://api.openai.org/v1", true),
            ("https://workspace.chatgpt-staging.com/backend-api", true),
            ("https://chatgpt-staging.com/backend-api", true),
        ] {
            assert_eq!(
                validate_runtime_environment(&runtime, Some(destination)).is_ok(),
                staging == expected_staging,
                "{chatgpt_base_url} -> {destination}",
            );
        }
    }
}
