use super::super::env_value;
use super::CredentialHostBinding;
use super::CredentialProvider;
use super::CredentialSource;
use super::shaped_dummy_value;
use crate::policy::normalize_host;
use base64::Engine as _;
use rama_http::HeaderMap;
use rama_http::HeaderValue;
use rama_http::header::AUTHORIZATION;
use std::collections::HashMap;

const GH_HOST_ENV_VAR: &str = "GH_HOST";
const GITHUB_TOKEN_PREFIXES: &[&str] = &["github_pat_", "ghp_", "gho_", "ghu_", "ghs_", "ghr_"];
const GITHUB_TOKEN_MIN_LEN: usize = 40;
const GITHUB_CLOUD_TOKEN_ENV_VARS: &[&str] = &["GH_TOKEN", "GITHUB_TOKEN"];
const GITHUB_ENTERPRISE_TOKEN_ENV_VARS: &[&str] =
    &["GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"];
const GITHUB_CLOUD_HOSTS: &[&str] = &["api.github.com", "github.com", "uploads.github.com"];
const GITHUB_CLOUD_HOST_SUFFIXES: &[&str] = &[".ghe.com"];

pub(super) static PROVIDER: CredentialProvider = CredentialProvider {
    context_env_vars: &[GH_HOST_ENV_VAR],
    sources: &[
        CredentialSource {
            env_vars: GITHUB_CLOUD_TOKEN_ENV_VARS,
            binding_env_vars: &[],
            host_binding: github_cloud_binding,
        },
        CredentialSource {
            env_vars: GITHUB_ENTERPRISE_TOKEN_ENV_VARS,
            binding_env_vars: &[GH_HOST_ENV_VAR],
            host_binding: github_enterprise_binding,
        },
    ],
    reset_on_configuration_change: false,
    dummy_value,
    translate_request_header,
    request_header_value,
    insert_request_header,
};

fn dummy_value(real_value: &str) -> String {
    shaped_dummy_value(
        real_value,
        github_token_prefix(real_value),
        GITHUB_TOKEN_MIN_LEN,
    )
}

fn request_header_value(value: &str) -> Option<HeaderValue> {
    HeaderValue::from_str(&format!("Bearer {value}")).ok()
}

fn insert_request_header(headers: &mut HeaderMap, value: HeaderValue) {
    headers.insert(AUTHORIZATION, value);
}

fn translate_request_header(
    headers: &HeaderMap,
    expected_value: &str,
    replacement_value: &str,
) -> Option<HeaderValue> {
    let header = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, value) = header.split_once(' ')?;
    let value = value.trim();
    if scheme.eq_ignore_ascii_case("basic") {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(value)
            .ok()?;
        let separator = decoded.iter().position(|byte| *byte == b':')?;
        let mut translated = Vec::with_capacity(decoded.len() + replacement_value.len());
        if &decoded[..separator] == expected_value.as_bytes() {
            translated.extend_from_slice(replacement_value.as_bytes());
            translated.extend_from_slice(&decoded[separator..]);
        } else if &decoded[separator + 1..] == expected_value.as_bytes() {
            translated.extend_from_slice(&decoded[..=separator]);
            translated.extend_from_slice(replacement_value.as_bytes());
        } else {
            return None;
        }
        let encoded = base64::engine::general_purpose::STANDARD.encode(translated);
        HeaderValue::from_str(&format!("{scheme} {encoded}")).ok()
    } else if (scheme.eq_ignore_ascii_case("bearer") || scheme.eq_ignore_ascii_case("token"))
        && value == expected_value
    {
        HeaderValue::from_str(&format!("{scheme} {replacement_value}")).ok()
    } else {
        None
    }
}

fn github_cloud_binding(
    _: &HashMap<String, String>,
    _: Option<&str>,
) -> Option<CredentialHostBinding> {
    Some(CredentialHostBinding::HostPattern {
        exact_hosts: GITHUB_CLOUD_HOSTS,
        suffixes: GITHUB_CLOUD_HOST_SUFFIXES,
    })
}

fn github_enterprise_binding(
    env: &HashMap<String, String>,
    _: Option<&str>,
) -> Option<CredentialHostBinding> {
    github_host_hint(env)
        .filter(|host| !github_cloud_host(host))
        .map(CredentialHostBinding::ExactHost)
}

fn github_cloud_host(host: &str) -> bool {
    GITHUB_CLOUD_HOSTS.contains(&host)
        || GITHUB_CLOUD_HOST_SUFFIXES
            .iter()
            .any(|suffix| host.ends_with(suffix))
}

fn github_token_prefix(value: &str) -> &str {
    GITHUB_TOKEN_PREFIXES
        .iter()
        .copied()
        .find(|prefix| value.starts_with(prefix))
        .unwrap_or("ghp_")
}

fn github_host_hint(env: &HashMap<String, String>) -> Option<String> {
    env_value(env, GH_HOST_ENV_VAR)
        .map(normalize_host)
        .filter(|host| !host.is_empty())
}
