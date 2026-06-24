use super::CredentialHostBinding;
use super::CredentialProvider;
use super::CredentialSource;
use super::shaped_dummy_value;
use rama_http::HeaderMap;
use rama_http::HeaderValue;
use rama_http::header::AUTHORIZATION;
use std::collections::HashMap;

const OPENAI_API_KEY_ENV_VARS: &[&str] = &["OPENAI_API_KEY"];
const OPENAI_API_KEY_MIN_LEN: usize = 51;
const OPENAI_API_HOST: &str = "api.openai.com";

pub(super) static PROVIDER: CredentialProvider = CredentialProvider {
    context_env_vars: &[],
    sources: &[CredentialSource {
        env_vars: OPENAI_API_KEY_ENV_VARS,
        host_binding,
    }],
    dummy_value,
    request_header,
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

fn request_header(headers: &HeaderMap) -> Option<&HeaderValue> {
    headers.get(AUTHORIZATION)
}

fn request_header_value(value: &str) -> Option<HeaderValue> {
    HeaderValue::from_str(&format!("Bearer {value}")).ok()
}

fn insert_request_header(headers: &mut HeaderMap, value: HeaderValue) {
    headers.insert(AUTHORIZATION, value);
}

fn host_binding(_: &HashMap<String, String>) -> Option<CredentialHostBinding> {
    Some(CredentialHostBinding::ExactHost(
        OPENAI_API_HOST.to_string(),
    ))
}

fn openai_api_key_prefix(value: &str) -> &str {
    let Some(suffix) = value.strip_prefix("sk-") else {
        return "sk-";
    };
    suffix
        .find('-')
        .map_or("sk-", |separator| &value[..separator + 4])
}
