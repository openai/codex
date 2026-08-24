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
    let authorization = format!("Bearer {github_token}");
    let mut env = env_map([
        ("GH_TOKEN", github_token),
        ("HOMEBREW_GITHUB_API_TOKEN", github_token),
        ("AUTH_HEADER", authorization.as_str()),
        ("OPENAI_API_KEY", openai_api_key),
        ("GH_ENTERPRISE_TOKEN", github_token),
    ]);

    broker.virtualize_child_env(&mut env);

    let github_dummy = env.get("GH_TOKEN").expect("dummy GitHub token");
    let openai_dummy = env.get("OPENAI_API_KEY").expect("dummy OpenAI API key");
    assert_credential_shape(github_token, github_dummy, "github_pat_");
    assert_credential_shape(openai_api_key, openai_dummy, "sk-proj-");
    assert_eq!(env.get("HOMEBREW_GITHUB_API_TOKEN"), Some(github_dummy));
    assert_eq!(env.get("GH_ENTERPRISE_TOKEN"), Some(github_dummy));
    assert_eq!(
        env.get("AUTH_HEADER"),
        Some(&format!("Bearer {github_dummy}"))
    );
    let mut persisted_credentials = format!("{github_token}\n{openai_api_key}");
    assert!(broker.virtualize_text(&mut persisted_credentials, &env));
    assert_eq!(
        persisted_credentials,
        format!("{github_dummy}\n{openai_dummy}")
    );
    let mut filtered_env = env.clone();
    filtered_env.remove("OPENAI_API_KEY");
    let mut excluded_credentials = format!("{github_token}\n{openai_api_key}");
    assert!(!broker.virtualize_text(&mut excluded_credentials, &filtered_env));
    assert_eq!(excluded_credentials, format!("{github_dummy}\n"));
    let mut excluded_dummies = format!("{github_dummy}\n{openai_dummy}");
    assert!(!broker.virtualize_text(&mut excluded_dummies, &filtered_env));
    assert_eq!(excluded_dummies, format!("{github_dummy}\n"));
    let mut command = vec![
        format!("Authorization: Bearer {github_dummy}"),
        format!("Authorization: Bearer {openai_dummy}"),
    ];
    let github_dummy = github_dummy.clone();
    let openai_dummy = openai_dummy.clone();
    env.insert("OPENAI_API_KEY".to_string(), "sk-user-override".to_string());
    env.insert(
        "GIT_CONFIG_VALUE_0".to_string(),
        format!("Authorization: Bearer {github_dummy}"),
    );
    assert_eq!(
        brokered_credential_dummy_env_keys(&env),
        vec!["GH_TOKEN".to_string()]
    );

    broker.restore_child_env(&mut env, &mut command);
    assert_eq!(env.get("GH_TOKEN").map(String::as_str), Some(github_token));
    assert_eq!(
        env.get("HOMEBREW_GITHUB_API_TOKEN").map(String::as_str),
        Some(github_token)
    );
    assert_eq!(
        env.get("GH_ENTERPRISE_TOKEN").map(String::as_str),
        Some(github_token)
    );
    assert_eq!(env.get("AUTH_HEADER"), Some(&authorization));
    assert_eq!(
        env.get("OPENAI_API_KEY").map(String::as_str),
        Some("sk-user-override")
    );
    assert_eq!(
        env.get("GIT_CONFIG_VALUE_0"),
        Some(&format!("Authorization: Bearer {github_dummy}"))
    );
    assert_eq!(
        command,
        vec![
            format!("Authorization: Bearer {github_dummy}"),
            format!("Authorization: Bearer {openai_dummy}"),
        ]
    );

    env.insert("GH_TOKEN".to_string(), openai_dummy.clone());
    env.insert("OPENAI_API_KEY".to_string(), github_dummy.clone());
    broker.restore_child_env(&mut env, &mut []);
    assert_eq!(env.get("GH_TOKEN"), Some(&openai_dummy));
    assert_eq!(env.get("OPENAI_API_KEY"), Some(&github_dummy));
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

    let mut alias_only = env_map([("HOMEBREW_GITHUB_API_TOKEN", "ghp-real-one")]);
    broker.virtualize_child_env(&mut alias_only);
    assert_eq!(
        alias_only.get("HOMEBREW_GITHUB_API_TOKEN"),
        Some(first_dummy)
    );
    broker.restore_child_env(&mut alias_only, &mut []);
    assert_eq!(alias_only["HOMEBREW_GITHUB_API_TOKEN"], "ghp-real-one");

    let mut overridden = env_map([
        ("GH_TOKEN", "ghp-real-two"),
        ("HOMEBREW_GITHUB_API_TOKEN", "ghp-real-one"),
    ]);
    broker.virtualize_child_env(&mut overridden);
    assert_eq!(overridden.get("GH_TOKEN"), Some(second_dummy));
    assert_eq!(
        overridden.get("HOMEBREW_GITHUB_API_TOKEN"),
        Some(first_dummy)
    );

    let mut cloud_alias = env_map([
        ("GH_TOKEN", "ghp-real-one"),
        ("GITHUB_TOKEN", "ghp-real-one"),
    ]);
    broker.virtualize_child_env(&mut cloud_alias);
    cloud_alias.insert("GITHUB_TOKEN".to_string(), first_dummy.clone());
    broker.restore_child_env(&mut cloud_alias, &mut []);
    assert_eq!(cloud_alias["GITHUB_TOKEN"], "ghp-real-one");

    let mut distinct_credentials = env_map([
        ("GH_TOKEN", "ghp-primary-secret"),
        ("GITHUB_TOKEN", "ghp-secondary-secret"),
    ]);
    broker.virtualize_child_env(&mut distinct_credentials);
    let secondary_dummy = distinct_credentials["GITHUB_TOKEN"].clone();
    distinct_credentials.remove("GITHUB_TOKEN");
    distinct_credentials.insert("GH_TOKEN".to_string(), secondary_dummy.clone());
    broker.virtualize_child_env(&mut distinct_credentials);
    broker.restore_child_env(&mut distinct_credentials, &mut []);
    assert_eq!(distinct_credentials["GH_TOKEN"], secondary_dummy);
}

#[test]
fn virtualize_child_env_replaces_aliases_of_filtered_parent_credentials() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    let github_token = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
    let authorization_header = format!("Bearer {github_token}");
    let parent_env = env_map([
        ("GH_TOKEN", github_token),
        ("HOMEBREW_GITHUB_API_TOKEN", github_token),
    ]);
    let mut child_env = env_map([
        ("HOMEBREW_GITHUB_API_TOKEN", github_token),
        ("AUTH_HEADER", authorization_header.as_str()),
    ]);

    broker.discover_parent_credentials(&parent_env, &child_env);
    broker.virtualize_child_env(&mut child_env);

    let dummy = child_env["HOMEBREW_GITHUB_API_TOKEN"].clone();
    assert_ne!(dummy, github_token);
    assert_eq!(child_env["AUTH_HEADER"], format!("Bearer {dummy}"));
    assert!(!child_env.contains_key("GH_TOKEN"));

    let mut headers = headers_with_bearer(&dummy);
    broker.inject_request_headers("api.github.com", &mut headers);
    assert_eq!(authorization(&headers), Some(authorization_header.as_str()));

    broker.restore_child_env(&mut child_env, &mut []);
    assert_eq!(child_env["HOMEBREW_GITHUB_API_TOKEN"], github_token);
    assert_eq!(child_env["AUTH_HEADER"], authorization_header);
    assert!(!child_env.contains_key("GH_TOKEN"));
}

#[test]
fn virtualize_child_env_binds_filtered_enterprise_credentials_to_child_host() {
    let github_token = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
    let authorization_header = format!("Bearer {github_token}");

    for (parent_host, include_cloud_token) in [
        (None, false),
        (Some("github.previous.example"), false),
        (None, true),
    ] {
        let broker = CredentialBroker::new(/*enabled*/ true);
        let mut parent_env = env_map([("GH_ENTERPRISE_TOKEN", github_token)]);
        if let Some(parent_host) = parent_host {
            parent_env.insert("GH_HOST".to_string(), parent_host.to_string());
        }
        if include_cloud_token {
            parent_env.insert("GH_TOKEN".to_string(), github_token.to_string());
        }
        let mut child_env = env_map([
            ("GH_HOST", "github.current.example"),
            ("AUTH_HEADER", authorization_header.as_str()),
        ]);

        broker.discover_parent_credentials(&parent_env, &child_env);
        broker.virtualize_child_env(&mut child_env);

        assert_ne!(child_env["AUTH_HEADER"], authorization_header);
        let mut headers = headers_with_authorization(&child_env["AUTH_HEADER"]);
        broker.inject_request_headers("github.current.example", &mut headers);
        assert_eq!(authorization(&headers), Some(authorization_header.as_str()));

        let mut previous_headers = headers_with_authorization(&child_env["AUTH_HEADER"]);
        broker.inject_request_headers("github.previous.example", &mut previous_headers);
        assert_eq!(
            authorization(&previous_headers),
            Some(child_env["AUTH_HEADER"].as_str())
        );
    }
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

    env.insert("GH_HOST".to_string(), "github.example.com".to_string());
    broker.virtualize_child_env(&mut env);
    let mut headers = headers_with_bearer(&env["GH_ENTERPRISE_TOKEN"]);
    broker.inject_request_headers("github.example.com", &mut headers);
    assert_eq!(authorization(&headers), Some("Bearer ghp-enterprise-real"));
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
    assert!(brokered_credential_binding_env_keys(&env).any(|key| key == "OPENAI_BASE_URL"));
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
    assert!(!brokered_credential_binding_env_keys(&env).any(|key| key == "GH_HOST"));
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
        ("GH_TOKEN", "ghp-enterprise-real"),
        ("GH_ENTERPRISE_TOKEN", "ghp-enterprise-real"),
        ("AUTH_HEADER", "Bearer ghp-enterprise-real"),
    ]);
    broker.virtualize_child_env(&mut env);
    let github_dummy = env["GH_TOKEN"].clone();
    assert!(brokered_credential_env_keys(&env).any(|key| key == "GH_HOST"));
    assert!(brokered_credential_binding_env_keys(&env).any(|key| key == "GH_HOST"));
    let github_token = env
        .get("GH_ENTERPRISE_TOKEN")
        .expect("dummy GitHub enterprise token");
    assert_ne!(github_token, &github_dummy);
    let mut headers = headers_with_bearer(github_token);

    broker.inject_request_headers("github.example.com", &mut headers);

    assert_eq!(authorization(&headers), Some("Bearer ghp-enterprise-real"));
    assert_eq!(env["AUTH_HEADER"], format!("Bearer {github_token}"));
    let mut alias_headers = headers_with_authorization(&env["AUTH_HEADER"]);
    broker.inject_request_headers("github.example.com", &mut alias_headers);
    assert_eq!(
        authorization(&alias_headers),
        Some("Bearer ghp-enterprise-real")
    );
    let mut persisted_alias = "Bearer ghp-enterprise-real".to_string();
    assert!(broker.virtualize_text(&mut persisted_alias, &env));
    assert_eq!(persisted_alias, format!("Bearer {github_token}"));
    assert_eq!(
        brokered_credential_dummy_env_keys(&env).first(),
        Some(&"GH_ENTERPRISE_TOKEN".to_string())
    );
    let mut cloud_headers = headers_with_bearer(&github_dummy);
    broker.inject_request_headers("github.example.com", &mut cloud_headers);
    assert_eq!(cloud_headers, headers_with_bearer(&github_dummy));
    let mut enterprise_headers = headers_with_bearer(github_token);
    broker.inject_request_headers("api.github.com", &mut enterprise_headers);
    assert_eq!(enterprise_headers, headers_with_bearer(github_token));
    let mut cloud_only = env_map([
        ("GH_HOST", "github.example.com"),
        ("GH_TOKEN", "ghp-enterprise-real"),
        ("AUTH_HEADER", "Bearer ghp-enterprise-real"),
    ]);
    broker.virtualize_child_env(&mut cloud_only);
    assert_eq!(cloud_only["AUTH_HEADER"], format!("Bearer {github_dummy}"));
    assert!(broker.host_requires_mitm("github.example.com"));
    assert!(broker.host_requires_mitm("api.github.com"));

    env.insert("GH_HOST".to_string(), "attacker.example".to_string());
    env.insert("GH_ENTERPRISE_TOKEN".to_string(), github_dummy.clone());
    broker.virtualize_child_env(&mut env);
    let mut attacker_headers = headers_with_bearer(&github_dummy);
    broker.inject_request_headers("attacker.example", &mut attacker_headers);
    assert_eq!(attacker_headers, headers_with_bearer(&github_dummy));
    assert!(!broker.host_requires_mitm("attacker.example"));

    let mut alternate_enterprise_key = env_map([
        ("GH_HOST", "github.alternate.example"),
        ("GH_TOKEN", "ghp-alternate-real"),
        ("GITHUB_ENTERPRISE_TOKEN", "ghp-alternate-real"),
        ("AUTH_HEADER", "Bearer ghp-alternate-real"),
    ]);
    broker.virtualize_child_env(&mut alternate_enterprise_key);
    assert_eq!(
        alternate_enterprise_key["AUTH_HEADER"],
        format!(
            "Bearer {}",
            alternate_enterprise_key["GITHUB_ENTERPRISE_TOKEN"]
        )
    );
    assert_eq!(
        brokered_credential_dummy_env_keys(&alternate_enterprise_key).first(),
        Some(&"GITHUB_ENTERPRISE_TOKEN".to_string())
    );
}
