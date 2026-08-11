use super::*;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use pretty_assertions::assert_eq;
use rama_http::HeaderValue;
use rama_http::header::AUTHORIZATION;

fn env_map<const N: usize>(entries: [(&str, &str); N]) -> HashMap<String, String> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn headers_with_bearer(value: &str) -> HeaderMap {
    headers_with_authorization(&format!("Bearer {value}"))
}

fn headers_with_authorization(value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(value).expect("valid authorization header"),
    );
    headers
}

fn authorization(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
}

fn assert_credential_shape(real_value: &str, dummy_value: &str, prefix: &str) {
    assert_ne!(dummy_value, real_value);
    assert_eq!(dummy_value.len(), real_value.len());
    assert_eq!(&dummy_value[..prefix.len()], prefix);
    let same_shape = real_value
        .bytes()
        .zip(dummy_value.bytes())
        .skip(prefix.len())
        .all(|(real, dummy)| {
            real.is_ascii_alphanumeric() && dummy.is_ascii_alphanumeric() || real == dummy
        });
    assert!(same_shape);
}

#[test]
fn virtualize_child_env_replaces_supported_credentials() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let github_token = "github_pat_11AA0bbCC_abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGH";
    let openai_api_key = "sk-proj-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
    let mut env = env_map([
        ("GH_TOKEN", github_token),
        ("OPENAI_API_KEY", openai_api_key),
        ("GH_ENTERPRISE_TOKEN", "ghp-enterprise-real"),
    ]);

    broker.virtualize_child_env(&mut env);

    let github_dummy = env.get("GH_TOKEN").expect("dummy GitHub token");
    let openai_dummy = env.get("OPENAI_API_KEY").expect("dummy OpenAI API key");
    assert_credential_shape(github_token, github_dummy, "github_pat_");
    assert_credential_shape(openai_api_key, openai_dummy, "sk-proj-");
    let mut command = vec![
        format!("Authorization: Bearer {github_dummy}"),
        format!("Authorization: Bearer {openai_dummy}"),
    ];
    let openai_dummy = openai_dummy.clone();
    env.insert("OPENAI_API_KEY".to_string(), "sk-user-override".to_string());
    assert_eq!(
        brokered_credential_dummy_env_keys(&env),
        vec!["GH_TOKEN".to_string()]
    );

    broker.restore_child_env(&mut env, &mut command);
    assert_eq!(env.get("GH_TOKEN").map(String::as_str), Some(github_token));
    assert_eq!(
        env.get("OPENAI_API_KEY").map(String::as_str),
        Some("sk-user-override")
    );
    assert_eq!(
        command,
        vec![
            format!("Authorization: Bearer {github_token}"),
            format!("Authorization: Bearer {openai_dummy}"),
        ]
    );
}

#[cfg(windows)]
#[test]
fn brokered_credentials_match_environment_keys_case_insensitively_on_windows() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let mut env = env_map([
        ("gh_host", "github.example.com"),
        ("gh_enterprise_token", "ghp-enterprise-real"),
    ]);

    broker.virtualize_child_env(&mut env);
    let dummy = env
        .get("GH_ENTERPRISE_TOKEN")
        .expect("dummy GitHub enterprise token");
    let mut headers = headers_with_bearer(dummy);
    broker.inject_request_headers("github.example.com", &mut headers);

    assert_eq!(
        brokered_credential_dummy_env_keys(&env),
        vec!["GH_ENTERPRISE_TOKEN".to_string()]
    );
    assert_eq!(authorization(&headers), Some("Bearer ghp-enterprise-real"));
}

#[test]
fn virtualize_child_env_preserves_live_dummy_mappings() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let mut first_env = env_map([("GH_TOKEN", "ghp-real-one")]);
    let mut second_env = env_map([("GH_TOKEN", "ghp-real-two")]);

    broker.virtualize_child_env(&mut first_env);
    broker.virtualize_child_env(&mut second_env);
    let first_dummy = first_env.get("GH_TOKEN").expect("first dummy token");
    let second_dummy = second_env.get("GH_TOKEN").expect("second dummy token");
    let mut first_headers = headers_with_bearer(first_dummy);
    let mut second_headers = headers_with_bearer(second_dummy);

    broker.inject_request_headers("api.github.com", &mut first_headers);
    broker.inject_request_headers("api.github.com", &mut second_headers);

    assert_eq!(authorization(&first_headers), Some("Bearer ghp-real-one"));
    assert_eq!(authorization(&second_headers), Some("Bearer ghp-real-two"));
}

#[test]
fn brokered_credential_env_keys_only_include_registered_credentials() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let mut env = env_map([
        ("OPENAI_API_KEY", "sk-real"),
        ("GH_TOKEN", ""),
        ("GH_HOST", "github.example.com"),
    ]);

    broker.virtualize_child_env(&mut env);
    env.insert(
        "GH_TOKEN".to_string(),
        "ghp_added_after_brokerage".to_string(),
    );

    assert_eq!(
        brokered_credential_env_keys(&env).collect::<Vec<_>>(),
        vec!["OPENAI_API_KEY"]
    );
}

#[test]
fn virtualize_child_env_uses_fresh_dummy_capabilities() {
    let mut first_env = env_map([("OPENAI_API_KEY", "sk-proj-abcdefghijklmnopqrstuvwxyz")]);
    let mut second_env = first_env.clone();

    CredentialBroker::new(/*enabled*/ true).virtualize_child_env(&mut first_env);
    CredentialBroker::new(/*enabled*/ true).virtualize_child_env(&mut second_env);

    assert_ne!(first_env["OPENAI_API_KEY"], second_env["OPENAI_API_KEY"]);
}

#[test]
fn child_without_dummy_cannot_use_previous_child_credential() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let mut first_env = env_map([("OPENAI_API_KEY", "sk-real")]);
    let mut second_env = HashMap::new();

    broker.virtualize_child_env(&mut first_env);
    broker.virtualize_child_env(&mut second_env);
    let mut headers = HeaderMap::new();

    broker.inject_request_headers("api.openai.com", &mut headers);

    assert_eq!(authorization(&headers), None);
}

#[test]
fn virtualize_child_env_preserves_unbound_enterprise_token() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let mut env = env_map([("GH_ENTERPRISE_TOKEN", "ghp-enterprise-real")]);

    broker.virtualize_child_env(&mut env);
    let inert_token = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
    let mut headers = headers_with_bearer(inert_token);
    broker.inject_request_headers("attacker.example", &mut headers);

    assert_eq!(env["GH_ENTERPRISE_TOKEN"], "ghp-enterprise-real");
    assert_eq!(headers, headers_with_bearer(inert_token));
    assert!(!broker.host_requires_mitm("attacker.example"));
}

#[test]
fn inject_request_headers_requires_dummy_to_select_ambiguous_github_credential() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let mut env = env_map([
        ("GH_TOKEN", "ghp-real-one"),
        ("GITHUB_TOKEN", "ghp-real-two"),
    ]);
    broker.virtualize_child_env(&mut env);
    let github_token = env.get("GITHUB_TOKEN").expect("dummy github token");
    let mut headers = HeaderMap::new();

    broker.inject_request_headers("api.github.com", &mut headers);
    assert_eq!(authorization(&headers), None);

    headers = headers_with_bearer(github_token);

    broker.inject_request_headers("api.github.com", &mut headers);

    assert_eq!(authorization(&headers), Some("Bearer ghp-real-two"));
}

#[test]
fn request_translation_preserves_provider_scheme_and_host_binding() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let mut env = env_map([("GH_TOKEN", "ghp-real")]);
    broker.virtualize_child_env(&mut env);
    let gh = &env["GH_TOKEN"];
    let basic_dummy = STANDARD.encode(format!("x-access-token:{gh}"));
    let basic_real = STANDARD.encode("x-access-token:ghp-real");
    let basic_username_dummy = STANDARD.encode(format!("{gh}:x-oauth-basic"));
    let basic_username_real = STANDARD.encode("ghp-real:x-oauth-basic");
    let basic_dummy = basic_dummy.as_str();
    let basic_real = basic_real.as_str();
    let basic_username_dummy = basic_username_dummy.as_str();
    let basic_username_real = basic_username_real.as_str();

    for (host, scheme, input, expected) in [
        ("github.com", "Basic", basic_dummy, basic_real),
        ("example.com", "Basic", basic_dummy, basic_dummy),
        (
            "github.com",
            "Basic",
            basic_username_dummy,
            basic_username_real,
        ),
        (
            "example.com",
            "Basic",
            basic_username_dummy,
            basic_username_dummy,
        ),
        ("api.github.com", "Bearer", gh.as_str(), "ghp-real"),
        ("uploads.github.com", "Bearer", gh.as_str(), "ghp-real"),
        ("api.github.com", "token", gh.as_str(), "ghp-real"),
    ] {
        let mut headers = headers_with_authorization(&format!("{scheme} {input}"));
        broker.inject_request_headers(host, &mut headers);
        let expected = format!("{scheme} {expected}");
        assert_eq!(authorization(&headers), Some(expected.as_str()), "{host}");
    }
}

#[test]
fn inject_request_headers_requires_dummy_and_preserves_explicit_authorization() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let mut env = env_map([("OPENAI_API_KEY", "sk-real")]);
    broker.virtualize_child_env(&mut env);
    let openai_api_key = env.get("OPENAI_API_KEY").expect("dummy OpenAI API key");
    let mut headers = HeaderMap::new();

    broker.inject_request_headers("api.openai.com", &mut headers);
    assert_eq!(authorization(&headers), None);

    headers = headers_with_bearer(openai_api_key);
    broker.inject_request_headers("api.openai.com", &mut headers);
    assert_eq!(authorization(&headers), Some("Bearer sk-real"));

    let mut explicit_headers = headers_with_bearer("sk-explicit");
    broker.inject_request_headers("api.openai.com", &mut explicit_headers);

    assert_eq!(authorization(&explicit_headers), Some("Bearer sk-explicit"));
}

#[test]
fn openai_credentials_bind_only_to_default_and_configured_trusted_hosts() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let mut config = NetworkProxyConfig::default();
    config.set_credential_broker_enabled(/*enabled*/ true);
    config.set_credential_broker_openai_base_url(
        /*base_url*/ Some("https://gateway.example.com./v1"),
    );
    broker.configure(&config);

    let mut env = env_map([
        ("OPENAI_API_KEY", "sk-real"),
        ("OPENAI_BASE_URL", "https://sdk.example.com./v1"),
        ("GH_TOKEN", "ghp-real"),
    ]);
    broker.virtualize_child_env(&mut env);
    assert!(brokered_credential_env_keys(&env).any(|key| key == "OPENAI_BASE_URL"));
    let dummy = &env["OPENAI_API_KEY"];

    for (host, expected_credential) in [
        ("api.openai.com", "sk-real"),
        ("gateway.example.com", "sk-real"),
        ("sdk.example.com", "sk-real"),
        ("attacker.example", dummy.as_str()),
    ] {
        let mut headers = headers_with_bearer(dummy);
        broker.inject_request_headers(host, &mut headers);
        let expected = format!("Bearer {expected_credential}");
        assert_eq!(authorization(&headers), Some(expected.as_str()), "{host}");
    }

    config.set_credential_broker_openai_base_url(
        /*base_url*/ Some("https://replacement.example/v1"),
    );
    broker.configure(&config);

    let mut github_headers = headers_with_bearer(&env["GH_TOKEN"]);
    broker.inject_request_headers("api.github.com", &mut github_headers);
    assert_eq!(authorization(&github_headers), Some("Bearer ghp-real"));

    let mut openai_headers = headers_with_bearer(dummy);
    broker.inject_request_headers("gateway.example.com", &mut openai_headers);
    assert_eq!(
        authorization(&openai_headers),
        Some(format!("Bearer {dummy}").as_str())
    );
}

#[test]
fn github_cloud_credentials_match_ghe_com_host_hint() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let mut env = env_map([("GH_HOST", "astemu.ghe.com"), ("GH_TOKEN", "ghp-real")]);
    broker.virtualize_child_env(&mut env);
    let github_token = env.get("GH_TOKEN").expect("dummy GitHub token");
    let mut headers = headers_with_bearer(github_token);

    broker.inject_request_headers("api.astemu.ghe.com", &mut headers);

    assert_eq!(authorization(&headers), Some("Bearer ghp-real"));
}

#[test]
fn github_cloud_credentials_do_not_bind_to_ghes_host_hint() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let mut env = env_map([("GH_HOST", "github.example.com"), ("GH_TOKEN", "ghp-real")]);
    broker.virtualize_child_env(&mut env);
    let github_token = env.get("GH_TOKEN").expect("dummy github token");
    let expected_authorization = format!("Bearer {github_token}");
    let mut headers = headers_with_bearer(github_token);

    broker.inject_request_headers("github.example.com", &mut headers);

    assert_eq!(
        authorization(&headers),
        Some(expected_authorization.as_str())
    );
    assert!(!broker.host_requires_mitm("github.example.com"));
    assert!(broker.host_requires_mitm("api.github.com"));
}

#[test]
fn github_enterprise_credentials_bind_to_gh_host() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let mut env = env_map([
        ("GH_HOST", "github.example.com"),
        ("GH_ENTERPRISE_TOKEN", "ghp-enterprise-real"),
    ]);
    broker.virtualize_child_env(&mut env);
    assert!(brokered_credential_env_keys(&env).any(|key| key == "GH_HOST"));
    let github_token = env
        .get("GH_ENTERPRISE_TOKEN")
        .expect("dummy GitHub enterprise token");
    let mut headers = headers_with_bearer(github_token);

    broker.inject_request_headers("github.example.com", &mut headers);

    assert_eq!(authorization(&headers), Some("Bearer ghp-enterprise-real"));
    assert!(broker.host_requires_mitm("github.example.com"));
    assert!(!broker.host_requires_mitm("api.github.com"));
}
