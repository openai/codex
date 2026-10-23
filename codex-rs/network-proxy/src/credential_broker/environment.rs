use super::BROKERED_CREDENTIAL_ALIAS_MARKER_PREFIX;
use super::BROKERED_CREDENTIALS_ENV_KEY;
use super::CREDENTIAL_BROKER_ACTIVE_ENV_KEY;
use super::CredentialBrokerState;
use super::MIN_EMBEDDED_CREDENTIAL_LENGTH;
use super::configured::valid_environment_key;
use super::env_entry;
use super::env_key_matches;
use super::env_value;
use super::matching;
use super::providers;
use super::registry::BrokeredCredentialProvider;
use super::remove_env_value;
use super::set_env_value;
use crate::NetworkProxy;
use crate::NetworkProxyConfig;
use crate::PreparedManagedNetwork;
use std::borrow::Cow;
use std::collections::HashMap;

/// Local destination hints retained by the broker, not added to child environments.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct CredentialBrokerContext(HashMap<String, String>);

impl From<HashMap<String, String>> for CredentialBrokerContext {
    fn from(env: HashMap<String, String>) -> Self {
        Self(env)
    }
}

impl std::fmt::Debug for CredentialBrokerContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CredentialBrokerContext(<redacted>)")
    }
}

impl CredentialBrokerContext {
    /// Uses private destination hints during brokerage and returns an environment ready to spawn.
    /// Context values override routing inputs, but never change the child's visible values.
    pub fn prepare_child_environment(
        self,
        proxy: &NetworkProxy,
        mut env: HashMap<String, String>,
        environment_id: Option<&str>,
    ) -> anyhow::Result<PreparedManagedNetwork> {
        let previous = self
            .0
            .into_iter()
            .map(|(key, value)| {
                let previous =
                    env_entry(&env, &key).map(|(key, value)| (key.to_string(), value.to_string()));
                set_env_value(&mut env, &key, value);
                (key, previous)
            })
            .collect::<Vec<_>>();
        let mut prepared = proxy.prepare_for_optional_environment(env, environment_id)?;
        for (key, previous) in previous {
            remove_env_value(&mut prepared.env, &key);
            if let Some((key, value)) = previous {
                prepared.env.insert(key, value);
            }
        }
        Ok(prepared)
    }

    pub(crate) fn capture(
        config: &NetworkProxyConfig,
        overrides: &HashMap<String, String>,
    ) -> Self {
        Self(
            credential_broker_provider_context_env_keys()
                .map(str::to_string)
                .chain(
                    config
                        .credential_providers
                        .values()
                        .filter_map(|provider| provider.url_prefix_from_env.clone()),
                )
                .filter_map(|key| {
                    env_value(overrides, &key)
                        .map(str::to_string)
                        .or_else(|| std::env::var(&key).ok())
                        .map(|value| (key, value))
                })
                .collect(),
        )
    }

    /// Resolves destination hints without applying or changing child environment policy.
    pub fn with_fallbacks<'a>(
        &self,
        env: &'a HashMap<String, String>,
    ) -> Cow<'a, HashMap<String, String>> {
        let mut context = Cow::Borrowed(env);
        for (key, value) in &self.0 {
            // A present value, including an empty one, overrides the trusted fallback.
            if env_value(&context, key).is_none() {
                set_env_value(context.to_mut(), key, value.clone());
            }
        }
        context
    }
}

/// Trusted provider metadata and active credential bindings for one child environment.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CredentialBrokerEnvironment {
    pub credential_keys: Vec<String>,
    pub binding_keys: Vec<String>,
    pub context_keys: Vec<String>,
    pub provider_keys: Vec<String>,
    pub provider_context_keys: Vec<String>,
    pub configured_provider_context_keys: Vec<String>,
}

impl CredentialBrokerState {
    pub(super) fn environment(&self, env: &HashMap<String, String>) -> CredentialBrokerEnvironment {
        let mut metadata = CredentialBrokerEnvironment::default();
        for key in
            providers::credential_env_keys().chain(credential_broker_provider_context_env_keys())
        {
            push_unique_key(&mut metadata.provider_keys, key);
        }
        for key in credential_broker_provider_context_env_keys() {
            push_unique_key(&mut metadata.provider_context_keys, key);
        }
        for provider in &self.configured_providers {
            for key in &provider.config.env {
                push_unique_key(&mut metadata.provider_keys, key);
            }
            if let Some(key) = provider.config.url_prefix_from_env.as_deref() {
                push_unique_key(&mut metadata.provider_keys, key);
                push_unique_key(&mut metadata.provider_context_keys, key);
                push_unique_key(&mut metadata.configured_provider_context_keys, key);
            }
        }

        if !self.enabled {
            return metadata;
        }

        metadata.credential_keys = brokered_credential_dummy_env_keys(env);
        let marked_credentials = env_value(env, BROKERED_CREDENTIALS_ENV_KEY)
            .and_then(|marker| serde_json::from_str::<Vec<(String, String)>>(marker).ok())
            .unwrap_or_default();
        for credential in &self.credentials {
            if matches!(
                credential.provider,
                BrokeredCredentialProvider::Configured(_)
            ) && env_value(env, &credential.env_var) == Some(credential.dummy_value.as_str())
                && marked_credentials.iter().any(|(key, value)| {
                    env_key_matches(key, &credential.env_var) && value == &credential.dummy_value
                })
            {
                push_unique_key(&mut metadata.credential_keys, &credential.env_var);
            }
        }

        for key in providers::credential_context_env_keys(&metadata.credential_keys) {
            if env_value(env, key).is_some() {
                push_unique_key(&mut metadata.context_keys, key);
            }
        }
        for key in providers::credential_binding_env_keys(&metadata.credential_keys) {
            if env_value(env, key).is_some() {
                push_unique_key(&mut metadata.binding_keys, key);
            }
        }
        for provider in &self.configured_providers {
            if provider.config.env.iter().any(|provider_key| {
                metadata
                    .credential_keys
                    .iter()
                    .any(|credential_key| env_key_matches(provider_key, credential_key))
            }) && let Some(key) = provider.config.url_prefix_from_env.as_deref()
                && env_value(env, key).is_some()
            {
                push_unique_key(&mut metadata.context_keys, key);
                push_unique_key(&mut metadata.binding_keys, key);
            }
        }

        metadata
    }
}

fn push_unique_key(keys: &mut Vec<String>, key: &str) {
    if !keys.iter().any(|candidate| env_key_matches(candidate, key)) {
        keys.push(key.to_string());
    }
}

pub(super) fn update_brokered_credentials_marker(
    state: &CredentialBrokerState,
    env: &mut HashMap<String, String>,
) {
    let credentials = state
        .credentials
        .iter()
        .filter(|credential| {
            env.iter().any(|(key, value)| {
                !env_key_matches(key, CREDENTIAL_BROKER_ACTIVE_ENV_KEY)
                    && !env_key_matches(key, BROKERED_CREDENTIALS_ENV_KEY)
                    && (value == &credential.dummy_value
                        || credential.contains_embedded_value(value, &credential.dummy_value))
            })
        })
        .collect::<Vec<_>>();
    let mut brokered = credentials
        .iter()
        .map(|credential| (credential.env_var.clone(), credential.dummy_value.clone()))
        .collect::<Vec<_>>();
    brokered.extend(state.credential_aliases.iter().flat_map(|alias| {
        credentials
            .iter()
            .filter(|credential| {
                alias.dummy_value == credential.dummy_value
                    || credential
                        .contains_embedded_value(&alias.dummy_value, &credential.dummy_value)
            })
            .map(|credential| {
                (
                    format!("{BROKERED_CREDENTIAL_ALIAS_MARKER_PREFIX}{}", alias.env_var),
                    credential.dummy_value.clone(),
                )
            })
    }));
    brokered.sort_unstable();
    brokered.dedup();
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
    let mut keys = marked_credential_dummy_env_keys(env)
        .into_iter()
        .filter(|key| {
            providers::credential_env_keys().any(|candidate| env_key_matches(key, candidate))
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

pub(crate) fn marked_credential_dummy_env_keys(env: &HashMap<String, String>) -> Vec<String> {
    env_value(env, BROKERED_CREDENTIALS_ENV_KEY)
        .and_then(|marker| serde_json::from_str::<Vec<(String, String)>>(marker).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(key, dummy_value)| {
            let (actual_key, actual_value) = env_entry(env, &key)?;
            (actual_value == dummy_value.as_str()).then(|| actual_key.to_string())
        })
        .collect()
}

/// Returns canonical credential and known alias keys recorded for an active brokered child,
/// including configured provider keys and keys that are currently absent.
pub fn brokered_credential_marker_env_keys(env: &HashMap<String, String>) -> Vec<String> {
    if env_value(env, CREDENTIAL_BROKER_ACTIVE_ENV_KEY) != Some("1") {
        return Vec::new();
    }
    let entries = env_value(env, BROKERED_CREDENTIALS_ENV_KEY)
        .and_then(|marker| serde_json::from_str::<Vec<(String, String)>>(marker).ok())
        .unwrap_or_default();
    let dummy_values = entries
        .iter()
        .filter(|(key, value)| valid_environment_key(key) && !value.is_empty())
        .map(|(_, dummy_value)| dummy_value.clone())
        .collect::<Vec<_>>();
    let mut keys = entries
        .into_iter()
        .filter_map(|(key, value)| {
            if valid_environment_key(&key) {
                return (!value.is_empty()).then_some(key);
            }
            let alias_key = key.strip_prefix(BROKERED_CREDENTIAL_ALIAS_MARKER_PREFIX)?;
            let known_alias = dummy_values
                .iter()
                .any(|dummy_value| value.contains(dummy_value));
            (known_alias && valid_environment_key(alias_key)).then(|| alias_key.to_string())
        })
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Returns environment keys whose current values are brokered dummies or aliases containing them,
/// including aliases retained after their canonical source variable was removed.
pub fn brokered_credential_value_env_keys(env: &HashMap<String, String>) -> Vec<String> {
    if env_value(env, CREDENTIAL_BROKER_ACTIVE_ENV_KEY) != Some("1") {
        return Vec::new();
    }
    let marked_values = env_value(env, BROKERED_CREDENTIALS_ENV_KEY)
        .and_then(|marker| serde_json::from_str::<Vec<(String, String)>>(marker).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|(key, dummy_value)| {
            !dummy_value.is_empty()
                && (valid_environment_key(key)
                    || key
                        .strip_prefix(BROKERED_CREDENTIAL_ALIAS_MARKER_PREFIX)
                        .is_some_and(valid_environment_key))
        })
        .collect::<Vec<_>>();
    let mut keys = env
        .iter()
        .filter(|(key, _)| {
            !env_key_matches(key, CREDENTIAL_BROKER_ACTIVE_ENV_KEY)
                && !env_key_matches(key, BROKERED_CREDENTIALS_ENV_KEY)
        })
        .filter(|(key, value)| {
            marked_values.iter().any(|(marked_key, dummy)| {
                value.as_str() == dummy.as_str()
                    || (dummy.len() >= MIN_EMBEDDED_CREDENTIAL_LENGTH
                        || marked_key
                            .strip_prefix(BROKERED_CREDENTIAL_ALIAS_MARKER_PREFIX)
                            .is_some_and(|alias_key| env_key_matches(key, alias_key)))
                        && value.contains(dummy.as_str())
            })
        })
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    keys.sort_unstable();
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

/// Checks whether each credential in a value has an allowed source environment key.
pub fn credential_broker_provider_sources_allowed(
    value: &str,
    virtualized: &str,
    source_env: &HashMap<String, String>,
    is_allowed: impl Fn(&str) -> bool,
) -> bool {
    let mut recognized = false;
    let allowed = providers::credential_providers()
        .filter(move |provider| {
            provider.credential_prefixes.iter().any(|prefix| {
                value.match_indices(*prefix).any(|(start, _)| {
                    matching::recognized_credential_match(provider, value, virtualized, start)
                        .is_some()
                })
            })
        })
        .all(|provider| {
            recognized = true;
            let actual_sources = provider
                .sources()
                .iter()
                .flat_map(|source| source.env_vars.iter().copied())
                .filter(|source| {
                    env_value(source_env, source).is_some_and(|source_value| {
                        source_value.len() >= provider.minimum_credential_len
                            && value.contains(source_value)
                    })
                })
                .collect::<Vec<_>>();
            let unattributed = provider.credential_prefixes.iter().any(|prefix| {
                value.match_indices(*prefix).any(|(start, _)| {
                    matching::recognized_credential_match(provider, value, virtualized, start)
                        .is_some_and(|credential| {
                            !actual_sources.iter().any(|source| {
                                env_value(source_env, source).is_some_and(|source_value| {
                                    credential == source_value
                                        || credential
                                            .strip_prefix(source_value)
                                            .is_some_and(|suffix| suffix.starts_with(['_', '-']))
                                })
                            })
                        })
                })
            });
            if actual_sources.is_empty() || unattributed {
                provider
                    .sources()
                    .iter()
                    .flat_map(|source| source.env_vars.iter().copied())
                    .all(&is_allowed)
            } else {
                actual_sources.iter().all(|source| {
                    actual_sources.iter().any(|equivalent| {
                        env_value(source_env, source) == env_value(source_env, equivalent)
                            && is_allowed(equivalent)
                    })
                })
            }
        });
    allowed && recognized
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
        if let Some(aws_keys) = crate::aws_sigv4::snapshot_env_keys(env) {
            keys.extend(aws_keys);
        }
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
