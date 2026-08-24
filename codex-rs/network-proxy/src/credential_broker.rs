mod providers;

use crate::config::NetworkProxyConfig;
use crate::policy::normalize_host;
use rama_http::HeaderMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

pub const CREDENTIAL_BROKER_ACTIVE_ENV_KEY: &str = "CODEX_NETWORK_PROXY_CREDENTIAL_BROKER_ACTIVE";
pub(crate) const BROKERED_CREDENTIALS_ENV_KEY: &str = "CODEX_NETWORK_PROXY_BROKERED_CREDENTIALS";
const MIN_EMBEDDED_CREDENTIAL_LENGTH: usize = 16;

#[derive(Clone)]
pub(crate) struct CredentialBroker {
    state: Arc<RwLock<CredentialBrokerState>>,
}

#[derive(Default)]
struct CredentialBrokerState {
    enabled: bool,
    openai_api_host: Option<String>,
    credentials: Vec<CredentialRecord>,
    credential_aliases: Vec<CredentialAlias>,
}

struct CredentialRecord {
    env_var: String,
    provider: &'static providers::CredentialProvider,
    host_binding: providers::CredentialHostBinding,
    real_value: String,
    dummy_value: String,
}

struct CredentialAlias {
    env_var: String,
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
            state.credential_aliases.clear();
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

    pub(crate) fn discover_parent_credentials(
        &self,
        parent_env: &HashMap<String, String>,
        child_env: &HashMap<String, String>,
    ) {
        let mut state = self.write_state();
        if !state.enabled {
            return;
        }

        for provider in providers::credential_providers() {
            for source in provider.sources() {
                let Some(host_binding) =
                    (source.host_binding)(child_env, state.openai_api_host.as_deref())
                else {
                    continue;
                };
                for env_var in source.env_vars {
                    let Some(real_value) =
                        brokerable_credential_value(parent_env, &state, env_var, provider)
                            .map(str::to_string)
                    else {
                        continue;
                    };
                    if env_value(child_env, env_var) == Some(real_value.as_str()) {
                        continue;
                    }
                    if child_env.values().any(|value| {
                        value == &real_value
                            || real_value.len() >= MIN_EMBEDDED_CREDENTIAL_LENGTH
                                && value.contains(&real_value)
                    }) {
                        state.register(env_var, provider, host_binding.clone(), &real_value);
                    }
                }
            }
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
                let Some(host_binding) =
                    (source.host_binding)(env, state.openai_api_host.as_deref())
                else {
                    continue;
                };
                for env_var in source.env_vars {
                    virtualize_env_var(env, &mut state, env_var, provider, host_binding.clone());
                }
            }
        }
        let credentials = prioritized_credentials(&state, env);
        let mut credential_aliases = Vec::new();
        for (key, value) in env.iter_mut() {
            let mut virtualized = false;
            for credential in &credentials {
                if value == &credential.real_value {
                    value.clone_from(&credential.dummy_value);
                    virtualized = true;
                } else if credential.real_value.len() >= MIN_EMBEDDED_CREDENTIAL_LENGTH
                    && value.contains(&credential.real_value)
                {
                    *value = value.replace(&credential.real_value, &credential.dummy_value);
                    virtualized = true;
                }
            }
            if virtualized {
                credential_aliases.push(CredentialAlias {
                    env_var: key.clone(),
                    dummy_value: value.clone(),
                });
            }
        }
        for alias in credential_aliases {
            if !state.credential_aliases.iter().any(|existing| {
                env_key_matches(&existing.env_var, &alias.env_var)
                    && existing.dummy_value == alias.dummy_value
            }) {
                state.credential_aliases.push(alias);
            }
        }
        update_brokered_credentials_marker(&state, env);
    }

    pub(crate) fn restore_child_env(
        &self,
        env: &mut HashMap<String, String>,
        _command: &mut [String],
    ) {
        let state = self.read_state();
        if !state.enabled || env_value(env, CREDENTIAL_BROKER_ACTIVE_ENV_KEY) != Some("1") {
            return;
        }

        let credentials = state
            .credentials
            .iter()
            .filter(|credential| {
                env.iter().any(|(key, value)| {
                    !env_key_matches(key, CREDENTIAL_BROKER_ACTIVE_ENV_KEY)
                        && !env_key_matches(key, BROKERED_CREDENTIALS_ENV_KEY)
                        && (value == &credential.dummy_value
                            || credential.real_value.len() >= MIN_EMBEDDED_CREDENTIAL_LENGTH
                                && value.contains(&credential.dummy_value))
                })
            })
            .collect::<Vec<_>>();
        for (key, value) in env.iter_mut() {
            if env_key_matches(key, CREDENTIAL_BROKER_ACTIVE_ENV_KEY)
                || env_key_matches(key, BROKERED_CREDENTIALS_ENV_KEY)
            {
                continue;
            }
            let canonical_credential = state
                .credentials
                .iter()
                .any(|credential| env_key_matches(key, &credential.env_var));
            if !canonical_credential
                && !state.credential_aliases.iter().any(|alias| {
                    env_key_matches(key, &alias.env_var) && value == &alias.dummy_value
                })
            {
                continue;
            }
            for credential in &credentials {
                if canonical_credential
                    && !state.credentials.iter().any(|candidate| {
                        env_key_matches(key, &candidate.env_var)
                            && std::ptr::eq(candidate.provider, credential.provider)
                            && candidate.host_binding == credential.host_binding
                            && candidate.real_value == credential.real_value
                    })
                {
                    continue;
                }
                if value == &credential.dummy_value {
                    value.clone_from(&credential.real_value);
                } else if credential.real_value.len() >= MIN_EMBEDDED_CREDENTIAL_LENGTH {
                    *value = value.replace(&credential.dummy_value, &credential.real_value);
                }
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

    pub(crate) fn virtualize_text(&self, text: &mut String, env: &HashMap<String, String>) -> bool {
        let state = self.read_state();
        if !state.enabled {
            return true;
        }

        let allowed_keys = brokered_credential_dummy_env_keys(env);
        let credentials = prioritized_credentials(&state, env);
        let mut allowed = true;
        for credential in &credentials {
            let contains_real = text.as_str() == credential.real_value
                || credential.real_value.len() >= MIN_EMBEDDED_CREDENTIAL_LENGTH
                    && text.contains(&credential.real_value);
            let contains_dummy = text.as_str() == credential.dummy_value
                || credential.dummy_value.len() >= MIN_EMBEDDED_CREDENTIAL_LENGTH
                    && text.contains(&credential.dummy_value);
            if !contains_real && !contains_dummy {
                continue;
            }

            let replacement = credentials.iter().copied().find(|candidate| {
                std::ptr::eq(candidate.provider, credential.provider)
                    && candidate.real_value == credential.real_value
                    && allowed_keys.iter().any(|key| {
                        env_key_matches(key, &candidate.env_var)
                            && env_value(env, key) == Some(candidate.dummy_value.as_str())
                    })
            });
            if replacement.is_none() {
                allowed = false;
            }
            let replacement = replacement.map_or("", |candidate| candidate.dummy_value.as_str());
            if contains_real {
                *text = text.replace(&credential.real_value, replacement);
            }
            if contains_dummy && credential.dummy_value != replacement {
                *text = text.replace(&credential.dummy_value, replacement);
            }
        }

        allowed
    }

    pub(crate) fn inject_request_headers(&self, host: &str, headers: &mut HeaderMap) {
        let normalized_host = normalize_host(host);
        let state = self.read_state();
        if !state.enabled {
            return;
        }

        let Some((credential, header_value)) =
            select_credential(headers, &normalized_host, &state.credentials)
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
    let Some(real_value) = brokerable_credential_value(env, state, env_var, provider)
        .map(str::to_string)
        .or_else(|| {
            let dummy = env_value(env, env_var)?;
            state
                .credentials
                .iter()
                .find(|credential| {
                    credential.dummy_value == dummy
                        && std::ptr::eq(credential.provider, provider)
                        && !env_key_matches(&credential.env_var, env_var)
                        && credential.host_binding == host_binding
                        && state.credentials.iter().any(|candidate| {
                            env_key_matches(&candidate.env_var, env_var)
                                && std::ptr::eq(candidate.provider, provider)
                                && candidate.host_binding == host_binding
                                && candidate.real_value == credential.real_value
                        })
                })
                .map(|credential| credential.real_value.clone())
        })
    else {
        return;
    };

    let dummy_value = state.register(env_var, provider, host_binding, &real_value);
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

fn prioritized_credentials<'a>(
    state: &'a CredentialBrokerState,
    env: &HashMap<String, String>,
) -> Vec<&'a CredentialRecord> {
    let mut credentials = state.credentials.iter().collect::<Vec<_>>();
    credentials.sort_unstable_by_key(|credential| {
        let active = env_entry(env, &credential.env_var)
            .is_some_and(|(_, value)| value == credential.dummy_value);
        let has_matching_host_binding = credential.provider.sources().iter().any(|source| {
            !source.binding_env_vars.is_empty()
                && (source.host_binding)(env, state.openai_api_host.as_deref())
                    .is_some_and(|binding| binding == credential.host_binding)
        });
        (
            std::cmp::Reverse(credential.real_value.len()),
            std::cmp::Reverse(active),
            std::cmp::Reverse(has_matching_host_binding),
        )
    });
    credentials
}

fn select_credential<'a>(
    headers: &HeaderMap,
    host: &str,
    credentials: &'a [CredentialRecord],
) -> Option<(&'a CredentialRecord, rama_http::HeaderValue)> {
    let mut translated_matches = credentials
        .iter()
        .filter(|credential| credential.matches_host(host))
        .filter_map(|credential| {
            credentials
                .iter()
                .filter(|candidate| {
                    std::ptr::eq(candidate.provider, credential.provider)
                        && candidate.real_value == credential.real_value
                        && candidate.matches_host(host)
                })
                .find_map(|candidate| {
                    credential.provider.translate_request_header(
                        headers,
                        &candidate.dummy_value,
                        &credential.real_value,
                    )
                })
                .map(|header_value| (credential, header_value))
        });
    let matched = translated_matches.next()?;
    translated_matches
        .all(|(credential, _)| {
            std::ptr::eq(credential.provider, matched.0.provider)
                && credential.real_value == matched.0.real_value
        })
        .then_some(matched)
}

fn update_brokered_credentials_marker(
    state: &CredentialBrokerState,
    env: &mut HashMap<String, String>,
) {
    let brokered = providers::credential_env_keys()
        .filter_map(|key| {
            let (actual_key, value) = env_entry(env, key)?;
            state
                .credentials
                .iter()
                .any(|credential| {
                    env_key_matches(actual_key, &credential.env_var)
                        && value == credential.dummy_value
                })
                .then_some((actual_key, value))
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
    let context_bound = |key: &str| {
        providers::credential_providers().any(|provider| {
            provider.sources().iter().any(|source| {
                source
                    .env_vars
                    .iter()
                    .any(|candidate| env_key_matches(key, candidate))
                    && source
                        .binding_env_vars
                        .iter()
                        .any(|context| env_value(env, context).is_some())
            })
        })
    };
    keys.sort_unstable_by(|left, right| {
        context_bound(right)
            .cmp(&context_bound(left))
            .then_with(|| left.cmp(right))
    });
    keys
}

/// Returns environment keys used to bind currently brokered credentials to hosts.
pub fn brokered_credential_binding_env_keys(
    env: &HashMap<String, String>,
) -> impl Iterator<Item = &'static str> {
    let brokered_keys = brokered_credential_dummy_env_keys(env);
    providers::credential_binding_env_keys(&brokered_keys)
        .filter(|key| env_value(env, key).is_some())
        .collect::<Vec<_>>()
        .into_iter()
}

/// Returns environment keys used to bind registered credential providers to their destinations.
pub fn credential_broker_provider_context_env_keys() -> impl Iterator<Item = &'static str> {
    providers::credential_providers().flat_map(|provider| provider.context_env_vars.iter().copied())
}

/// Returns whether an environment key belongs to a supported credential provider.
pub fn is_credential_broker_provider_env_key(key: &str) -> bool {
    providers::credential_providers().any(|provider| {
        provider
            .sources()
            .iter()
            .flat_map(|source| source.env_vars.iter().copied())
            .chain(provider.context_env_vars.iter().copied())
            .any(|candidate| env_key_matches(key, candidate))
    })
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
