use code_core::{built_in_model_providers, WireApi};
use serial_test::serial;

fn with_env_override<F, R>(key: &str, value: Option<&str>, f: F) -> R
where
    F: FnOnce() -> R,
{
    let original = std::env::var(key).ok();
    set_env_guarded(key, value);

    let result = f();

    set_env_guarded(key, original.as_deref());

    result
}

fn set_env_guarded(key: &str, value: Option<&str>) {
    match value {
        Some(v) => unsafe {
            // SAFETY: each test uses the `serial` attribute so we never update
            // environment variables concurrently within the process.
            std::env::set_var(key, v)
        },
        None => unsafe {
            // SAFETY: see above; we restore or clear the variable while
            // holding the same process-wide lock enforced by `serial`.
            std::env::remove_var(key)
        },
    }
}

fn openai_provider_wire_api() -> WireApi {
    let providers = built_in_model_providers();
    providers
        .get("openai")
        .unwrap_or_else(|| panic!("missing built-in openai provider"))
        .wire_api
}

#[test]
#[serial]
fn openai_wire_api_defaults_to_responses() {
    let wire_api = with_env_override("OPENAI_WIRE_API", None, openai_provider_wire_api);
    assert_eq!(wire_api, WireApi::Responses);
}

#[test]
#[serial]
fn openai_wire_api_env_chat() {
    let wire_api = with_env_override("OPENAI_WIRE_API", Some("chat"), openai_provider_wire_api);
    assert_eq!(wire_api, WireApi::Chat);
}

#[test]
#[serial]
fn openai_wire_api_env_responses() {
    let wire_api = with_env_override(
        "OPENAI_WIRE_API",
        Some("responses"),
        openai_provider_wire_api,
    );
    assert_eq!(wire_api, WireApi::Responses);
}

#[test]
#[serial]
fn openai_wire_api_env_invalid_falls_back_to_responses() {
    let wire_api = with_env_override(
        "OPENAI_WIRE_API",
        Some("invalid-mode"),
        openai_provider_wire_api,
    );
    assert_eq!(wire_api, WireApi::Responses);
}

#[test]
#[serial]
fn openai_wire_api_env_chat_is_case_insensitive_and_tolerates_whitespace() {
    let wire_api = with_env_override(
        "OPENAI_WIRE_API",
        Some("  CHAT  "),
        openai_provider_wire_api,
    );
    assert_eq!(wire_api, WireApi::Chat);
}
