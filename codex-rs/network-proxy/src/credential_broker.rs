mod providers;

use crate::config::NetworkProxyConfig;
use crate::policy::normalize_host;
use rama_http::HeaderMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

pub const CREDENTIAL_BROKER_ACTIVE_ENV_KEY: &str = "CODEX_NETWORK_PROXY_CREDENTIAL_BROKER_ACTIVE";
pub(crate) const BROKERED_CREDENTIALS_ENV_KEY: &str = "CODEX_NETWORK_PROXY_BROKERED_CREDENTIALS";

#[derive(Clone)]
pub(crate) struct CredentialBroker {
    state: Arc<RwLock<CredentialBrokerState>>,
}

#[derive(Default)]
struct CredentialBrokerState {
    enabled: bool,
    openai_api_host: Option<String>,
    credentials: Vec<CredentialRecord>,
}

struct CredentialRecord {
    env_var: String,
    provider: &'static providers::CredentialProvider,
    host_binding: providers::CredentialHostBinding,
    real_value: String,
    dummy_value: String,
}

fn env_key_matches(candidate: &str, expected: &str) -> bool {
    if cfg!(windows) {
        candidate.eq_ignore_ascii_case(expected)
    } else {
        candidate == expected
    }
}

fn env_entry<'a>(env: &'a HashMap<String, String>, key: &str) -> Option<(&'a str, &'a str)> {
    env.iter()
        .find(|(candidate, _)| env_key_matches(candidate, key))
        .map(|(key, value)| (key.as_str(), value.as_str()))
}

pub(super) fn env_value<'a>(env: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    env_entry(env, key).map(|(_, value)| value)
}

fn set_env_value(env: &mut HashMap<String, String>, key: &str, value: String) {
    if cfg!(windows) {
        env.retain(|candidate, _| !env_key_matches(candidate, key));
    }
    env.insert(key.to_string(), value);
}

fn remove_env_value(env: &mut HashMap<String, String>, key: &str) {
    if cfg!(windows) {
        env.retain(|candidate, _| !env_key_matches(candidate, key));
    } else {
        env.remove(key);
    }
}

impl CredentialBroker {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            state: Arc::new(RwLock::new(CredentialBrokerState {
                enabled,
                ..CredentialBrokerState::default()
            })),
        }
    }

    pub(crate) fn configure(&self, config: &NetworkProxyConfig) {
        let mut state = self.write_state();
        if state.enabled != config.credential_broker {
            state.enabled = config.credential_broker;
            state.credentials.clear();
        }
        if state.openai_api_host != config.credential_broker_openai_host {
            state
                .openai_api_host
                .clone_from(&config.credential_broker_openai_host);
            state
                .credentials
                .retain(|credential| !credential.provider.reset_on_configuration_change);
        }
    }

    pub(crate) fn virtualize_child_env(&self, env: &mut HashMap<String, String>) {
        let mut state = self.write_state();
        if !state.enabled {
            remove_env_value(env, CREDENTIAL_BROKER_ACTIVE_ENV_KEY);
            remove_env_value(env, BROKERED_CREDENTIALS_ENV_KEY);
            return;
        }
        set_env_value(env, CREDENTIAL_BROKER_ACTIVE_ENV_KEY, "1".to_string());

        for provider in providers::credential_providers() {
            for source in provider.sources() {
                if let Some(host_binding) =
                    (source.host_binding)(env, state.openai_api_host.as_deref())
                {
                    for env_var in source.env_vars {
                        virtualize_env_var(
                            env,
                            &mut state,
                            env_var,
                            provider,
                            host_binding.clone(),
                        );
                    }
                }
            }
        }
        update_brokered_credentials_marker(&state, env);
    }

    pub(crate) fn restore_child_env(
        &self,
        env: &mut HashMap<String, String>,
        command: &mut [String],
    ) {
        let state = self.read_state();
        if !state.enabled || env_value(env, CREDENTIAL_BROKER_ACTIVE_ENV_KEY) != Some("1") {
            return;
        }

        for (key, value) in env {
            if let Some(credential) = state.credentials.iter().find(|credential| {
                env_key_matches(key, &credential.env_var) && value == &credential.dummy_value
            }) {
                for argument in command.iter_mut() {
                    if argument.contains(&credential.dummy_value) {
                        *argument =
                            argument.replace(&credential.dummy_value, &credential.real_value);
                    }
                }
                *value = credential.real_value.clone();
            }
        }
    }

    pub(crate) fn host_requires_mitm(&self, host: &str) -> bool {
        let normalized_host = normalize_host(host);
        let state = self.read_state();
        state.enabled
            && state
                .credentials
                .iter()
                .any(|credential| credential.matches_host(&normalized_host))
    }

    pub(crate) fn inject_request_headers(&self, host: &str, headers: &mut HeaderMap) {
        let normalized_host = normalize_host(host);
        let state = self.read_state();
        if !state.enabled {
            return;
        }

        let matching_credentials = state
            .credentials
            .iter()
            .filter(|credential| credential.matches_host(&normalized_host))
            .collect::<Vec<_>>();
        let Some((credential, header_value)) = select_credential(headers, &matching_credentials)
        else {
            return;
        };
        credential
            .provider
            .insert_request_header(headers, header_value);
    }

    fn read_state(&self) -> std::sync::RwLockReadGuard<'_, CredentialBrokerState> {
        self.state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write_state(&self) -> std::sync::RwLockWriteGuard<'_, CredentialBrokerState> {
        self.state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn virtualize_env_var(
    env: &mut HashMap<String, String>,
    state: &mut CredentialBrokerState,
    env_var: &str,
    provider: &'static providers::CredentialProvider,
    host_binding: providers::CredentialHostBinding,
) {
    let Some(real_value) = brokerable_credential_value(env, state, env_var, provider) else {
        return;
    };

    let dummy_value = state.register(env_var, provider, host_binding, real_value);
    set_env_value(env, env_var, dummy_value);
}

fn brokerable_credential_value<'a>(
    env: &'a HashMap<String, String>,
    state: &CredentialBrokerState,
    env_var: &str,
    provider: &providers::CredentialProvider,
) -> Option<&'a str> {
    let real_value = env_value(env, env_var)?.trim();
    (!real_value.is_empty()
        && !state.is_dummy_value(real_value)
        && provider.request_header_value(real_value).is_some())
    .then_some(real_value)
}

impl CredentialBrokerState {
    fn register(
        &mut self,
        env_var: &str,
        provider: &'static providers::CredentialProvider,
        host_binding: providers::CredentialHostBinding,
        real_value: &str,
    ) -> String {
        if let Some(existing) = self.credentials.iter().find(|credential| {
            credential.env_var == env_var
                && std::ptr::eq(credential.provider, provider)
                && credential.host_binding == host_binding
                && credential.real_value == real_value
        }) {
            return existing.dummy_value.clone();
        }

        let dummy_value = loop {
            let candidate = provider.dummy_value(real_value);
            if candidate != real_value && !self.is_dummy_value(&candidate) {
                break candidate;
            }
        };
        self.credentials.push(CredentialRecord {
            env_var: env_var.to_string(),
            provider,
            host_binding,
            real_value: real_value.to_string(),
            dummy_value: dummy_value.clone(),
        });
        dummy_value
    }

    fn is_dummy_value(&self, value: &str) -> bool {
        self.credentials
            .iter()
            .any(|credential| credential.dummy_value == value)
    }
}

impl CredentialRecord {
    fn matches_host(&self, host: &str) -> bool {
        self.host_binding.matches_host(host)
    }
}

fn select_credential<'a>(
    headers: &HeaderMap,
    matching_credentials: &[&'a CredentialRecord],
) -> Option<(&'a CredentialRecord, rama_http::HeaderValue)> {
    let mut translated_matches = matching_credentials
        .iter()
        .copied()
        .filter_map(|credential| {
            credential
                .provider
                .translate_request_header(headers, &credential.dummy_value, &credential.real_value)
                .map(|header_value| (credential, header_value))
        });
    let matched = translated_matches.next()?;
    translated_matches.next().is_none().then_some(matched)
}

fn update_brokered_credentials_marker(
    state: &CredentialBrokerState,
    env: &mut HashMap<String, String>,
) {
    let brokered = providers::credential_env_keys()
        .filter_map(|key| {
            let (actual_key, value) = env_entry(env, key)?;
            state.is_dummy_value(value).then_some((actual_key, value))
        })
        .collect::<Vec<_>>();
    match serde_json::to_string(&brokered) {
        Ok(marker) => {
            set_env_value(env, BROKERED_CREDENTIALS_ENV_KEY, marker);
        }
        Err(_) => {
            remove_env_value(env, BROKERED_CREDENTIALS_ENV_KEY);
        }
    }
}

/// Returns supported environment keys whose current values still match the child-scoped dummy
/// values recorded by the credential broker.
///
/// The broker marker is treated as untrusted: malformed metadata, unsupported keys, and values
/// replaced by the user are ignored. The environment is not mutated; callers own the decision to
/// remove the returned keys.
pub fn brokered_credential_dummy_env_keys(env: &HashMap<String, String>) -> Vec<String> {
    let mut keys = env_value(env, BROKERED_CREDENTIALS_ENV_KEY)
        .and_then(|marker| serde_json::from_str::<Vec<(String, String)>>(marker).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(key, dummy_value)| {
            providers::credential_env_keys()
                .any(|candidate| env_key_matches(&key, candidate))
                .then_some(())?;
            let (actual_key, actual_value) = env_entry(env, &key)?;
            (actual_value == dummy_value.as_str()).then(|| actual_key.to_string())
        })
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

/// Returns credential keys plus provider context keys already present in an environment with an
/// active broker.
pub fn brokered_credential_env_keys(
    env: &HashMap<String, String>,
) -> impl Iterator<Item = &'static str> {
    let active = env_value(env, CREDENTIAL_BROKER_ACTIVE_ENV_KEY).is_some_and(|value| value == "1");
    let mut keys = Vec::new();
    if active {
        let brokered_keys = brokered_credential_dummy_env_keys(env);
        keys.extend(providers::credential_env_keys().filter(|key| {
            brokered_keys
                .iter()
                .any(|brokered_key| env_key_matches(brokered_key, key))
        }));
        keys.extend(
            providers::credential_context_env_keys(&brokered_keys)
                .filter(|key| env_value(env, key).is_some()),
        );
    }
    keys.into_iter()
}

#[cfg(test)]
#[path = "credential_broker_tests.rs"]
mod tests;
