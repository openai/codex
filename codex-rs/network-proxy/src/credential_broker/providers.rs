use crate::policy::normalize_host;
use rama_http::HeaderMap;
use rama_http::HeaderValue;
use rama_http::header::AUTHORIZATION;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;

const GH_HOST_ENV_VAR: &str = "GH_HOST";
const GITHUB_TOKEN_PREFIXES: &[&str] = &["github_pat_", "ghp_", "gho_", "ghu_", "ghs_", "ghr_"];
const GITHUB_TOKEN_MIN_LEN: usize = 40;
const OPENAI_API_KEY_MIN_LEN: usize = 51;
const GITHUB_CLOUD_TOKEN_ENV_VARS: &[&str] = &["GH_TOKEN", "GITHUB_TOKEN"];
const GITHUB_ENTERPRISE_TOKEN_ENV_VARS: &[&str] =
    &["GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"];
const OPENAI_API_KEY_ENV_VARS: &[&str] = &["OPENAI_API_KEY"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CredentialKind {
    GitHub,
    OpenAiApiKey,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum CredentialHostBinding {
    GitHubCloud,
    ExactHost(String),
    OpenAiApi,
}

pub(super) struct CredentialSource {
    pub(super) env_vars: &'static [&'static str],
    pub(super) kind: CredentialKind,
    pub(super) host_binding: fn(&HashMap<String, String>) -> Option<CredentialHostBinding>,
}

pub(super) const CREDENTIAL_SOURCES: &[CredentialSource] = &[
    CredentialSource {
        env_vars: GITHUB_CLOUD_TOKEN_ENV_VARS,
        kind: CredentialKind::GitHub,
        host_binding: github_cloud_binding,
    },
    CredentialSource {
        env_vars: GITHUB_ENTERPRISE_TOKEN_ENV_VARS,
        kind: CredentialKind::GitHub,
        host_binding: github_enterprise_binding,
    },
    CredentialSource {
        env_vars: OPENAI_API_KEY_ENV_VARS,
        kind: CredentialKind::OpenAiApiKey,
        host_binding: openai_api_binding,
    },
];

impl CredentialKind {
    pub(super) fn dummy_value(self, credential_id: usize, real_value: &str) -> String {
        match self {
            Self::GitHub => shaped_dummy_value(
                real_value,
                github_token_prefix(real_value),
                GITHUB_TOKEN_MIN_LEN,
                "github",
                credential_id,
            ),
            Self::OpenAiApiKey => shaped_dummy_value(
                real_value,
                openai_api_key_prefix(real_value),
                OPENAI_API_KEY_MIN_LEN,
                "openai",
                credential_id,
            ),
        }
    }

    pub(super) fn request_header(self, headers: &HeaderMap) -> Option<&HeaderValue> {
        match self {
            Self::GitHub | Self::OpenAiApiKey => headers.get(AUTHORIZATION),
        }
    }

    pub(super) fn request_header_value(self, value: &str) -> Option<HeaderValue> {
        match self {
            Self::GitHub | Self::OpenAiApiKey => {
                HeaderValue::from_str(&format!("Bearer {value}")).ok()
            }
        }
    }

    pub(super) fn insert_request_header(self, headers: &mut HeaderMap, value: HeaderValue) {
        match self {
            Self::GitHub | Self::OpenAiApiKey => {
                headers.insert(AUTHORIZATION, value);
            }
        }
    }
}

impl CredentialHostBinding {
    pub(super) fn matches_host(&self, host: &str) -> bool {
        match self {
            Self::GitHubCloud => github_cloud_host(host),
            Self::ExactHost(expected_host) => host == expected_host,
            Self::OpenAiApi => host == "api.openai.com",
        }
    }
}

pub(super) fn credential_broker_env_keys() -> impl Iterator<Item = &'static str> {
    std::iter::once(GH_HOST_ENV_VAR).chain(
        CREDENTIAL_SOURCES
            .iter()
            .flat_map(|source| source.env_vars.iter().copied()),
    )
}

fn github_cloud_host(host: &str) -> bool {
    matches!(host, "api.github.com" | "github.com") || host.ends_with(".ghe.com")
}

fn github_token_prefix(value: &str) -> &str {
    GITHUB_TOKEN_PREFIXES
        .iter()
        .copied()
        .find(|prefix| value.starts_with(prefix))
        .unwrap_or("ghp_")
}

fn openai_api_key_prefix(value: &str) -> &str {
    let Some(suffix) = value.strip_prefix("sk-") else {
        return "sk-";
    };
    suffix
        .find('-')
        .map_or("sk-", |separator| &value[..separator + 4])
}

fn shaped_dummy_value(
    real_value: &str,
    prefix: &str,
    minimum_len: usize,
    seed: &str,
    credential_id: usize,
) -> String {
    let target_len = real_value.len().max(minimum_len).max(prefix.len() + 16);
    let digest = Sha256::digest(format!("{seed}:{credential_id}").as_bytes());
    let mut dummy = String::with_capacity(target_len);
    dummy.push_str(prefix);
    for index in prefix.len()..target_len {
        let offset = index - prefix.len();
        let entropy = digest[offset % digest.len()].wrapping_add(offset as u8);
        let character = match real_value.as_bytes().get(index).copied() {
            Some(template) if !template.is_ascii_alphanumeric() => template,
            Some(template) if template.is_ascii_digit() => b'0' + entropy % 10,
            Some(template) if template.is_ascii_uppercase() => b'A' + entropy % 26,
            _ => b'a' + entropy % 26,
        };
        dummy.push(char::from(character));
    }
    dummy
}

fn github_cloud_binding(_: &HashMap<String, String>) -> Option<CredentialHostBinding> {
    Some(CredentialHostBinding::GitHubCloud)
}

fn github_enterprise_binding(env: &HashMap<String, String>) -> Option<CredentialHostBinding> {
    github_host_hint(env)
        .filter(|host| !github_cloud_host(host))
        .map(CredentialHostBinding::ExactHost)
}

fn openai_api_binding(_: &HashMap<String, String>) -> Option<CredentialHostBinding> {
    Some(CredentialHostBinding::OpenAiApi)
}

fn github_host_hint(env: &HashMap<String, String>) -> Option<String> {
    env.get(GH_HOST_ENV_VAR)
        .map(String::as_str)
        .map(normalize_host)
        .filter(|host| !host.is_empty())
}
