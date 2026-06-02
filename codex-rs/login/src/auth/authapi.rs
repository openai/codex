use std::env;

const PROD_AUTHAPI_BASE_URL: &str = "https://auth.openai.com/api/accounts";
const CODEX_AUTHAPI_BASE_URL_ENV_VAR: &str = "CODEX_AUTHAPI_BASE_URL";
const LEGACY_CODEX_AGENT_IDENTITY_AUTHAPI_BASE_URL_ENV_VAR: &str =
    "CODEX_AGENT_IDENTITY_AUTHAPI_BASE_URL";

pub(super) fn authapi_base_url() -> String {
    read_non_empty_env_var(CODEX_AUTHAPI_BASE_URL_ENV_VAR)
        .or_else(|| read_non_empty_env_var(LEGACY_CODEX_AGENT_IDENTITY_AUTHAPI_BASE_URL_ENV_VAR))
        .unwrap_or_else(|| PROD_AUTHAPI_BASE_URL.to_string())
}

fn read_non_empty_env_var(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|base_url| base_url.trim().trim_end_matches('/').to_string())
        .filter(|base_url| !base_url.is_empty())
}
