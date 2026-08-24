use super::super::env_value;
use super::CredentialHostBinding;
use super::CredentialProvider;
use super::CredentialSource;
use super::shaped_dummy_value;
use crate::config::trusted_credential_broker_host;
use rama_http::HeaderMap;
use rama_http::HeaderValue;
use rama_http::header::AUTHORIZATION;
use std::collections::HashMap;

const OPENAI_API_KEY_ENV_VARS: &[&str] = &["OPENAI_API_KEY"];
const OPENAI_BASE_URL_ENV_VAR: &str = "OPENAI_BASE_URL";
const OPENAI_API_KEY_MIN_LEN: usize = 51;
const OPENAI_API_HOST: &str = "api.openai.com";

pub(super) static PROVIDER: CredentialProvider = CredentialProvider {
    context_env_vars: &[OPENAI_BASE_URL_ENV_VAR],
    sources: &[CredentialSource {
        env_vars: OPENAI_API_KEY_ENV_VARS,
        binding_env_vars: &[OPENAI_BASE_URL_ENV_VAR],
        host_binding,
    }],
    reset_on_configuration_change: true,
    dummy_value,
    translate_request_header,
    request_header_value,
    insert_request_header,
};

fn dummy_value(real_value: &str) -> String {
    shaped_dummy_value(
        real_value,
        openai_api_key_prefix(real_value),
        OPENAI_API_KEY_MIN_LEN,
    )
}

fn translate_request_header(
    headers: &HeaderMap,
    expected_value: &str,
    replacement_value: &str,
) -> Option<HeaderValue> {
    headers
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .filter(|header| header.contains(expected_value))?;
    request_header_value(replacement_value)
}

fn request_header_value(value: &str) -> Option<HeaderValue> {
    HeaderValue::from_str(&format!("Bearer {value}")).ok()
}

fn insert_request_header(headers: &mut HeaderMap, value: HeaderValue) {
    headers.insert(AUTHORIZATION, value);
}

fn host_binding(
    env: &HashMap<String, String>,
    configured_host: Option<&str>,
) -> Option<CredentialHostBinding> {
    let mut hosts = vec![OPENAI_API_HOST.to_string()];
    for host in configured_host
        .map(str::to_string)
        .into_iter()
        .chain(env_value(env, OPENAI_BASE_URL_ENV_VAR).and_then(trusted_credential_broker_host))
    {
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }

    Some(match hosts.as_slice() {
        [host] => CredentialHostBinding::ExactHost(host.clone()),
        _ => CredentialHostBinding::ExactHosts(hosts),
    })
}

fn openai_api_key_prefix(value: &str) -> &str {
    let Some(suffix) = value.strip_prefix("sk-") else {
        return "sk-";
    };
    suffix
        .find('-')
        .map_or("sk-", |separator| &value[..separator + 4])
}
