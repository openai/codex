use super::CredentialBrokerState;
use super::CredentialRecord;
use super::MIN_EMBEDDED_CREDENTIAL_LENGTH;
use super::configured::ConfiguredCredentialProvider;
use super::env_entry;
use super::env_key_matches;
use super::env_value;
use super::matching;
use super::matching::is_operational_path_match;
use super::providers;
use super::source_accepts_credential;
use super::source_tracks_credential;
use crate::policy::normalize_host;
use base64::Engine;
use rama_http::HeaderMap;
use rama_http::HeaderName;
use rama_http::HeaderValue;
use rama_http::header::AUTHORIZATION;
use std::collections::HashMap;
use std::sync::Arc;
use url::Url;

#[derive(Clone)]
pub(super) enum BrokeredCredentialProvider {
    Builtin(&'static providers::CredentialProvider),
    Configured(Arc<ConfiguredCredentialProvider>),
}

#[derive(Clone)]
pub(super) struct ActiveCredentialSource {
    pub(super) provider: BrokeredCredentialProvider,
    pub(super) host_binding: providers::CredentialHostBinding,
    pub(super) env_vars: Vec<String>,
}

impl BrokeredCredentialProvider {
    pub(super) fn same_provider(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Builtin(left), Self::Builtin(right)) => std::ptr::eq(*left, *right),
            (Self::Configured(left), Self::Configured(right)) => Arc::ptr_eq(left, right),
            (Self::Builtin(_), Self::Configured(_)) | (Self::Configured(_), Self::Builtin(_)) => {
                false
            }
        }
    }

    pub(super) fn dummy_value(&self, real_value: &str) -> Option<String> {
        match self {
            Self::Builtin(provider) => Some(provider.dummy_value(real_value)),
            Self::Configured(provider) => provider.dummy_value(real_value),
        }
    }

    pub(super) fn recognizes_credential_in(&self, value: &str) -> bool {
        match self {
            Self::Builtin(provider) => provider.credential_prefixes.iter().any(|prefix| {
                value.match_indices(*prefix).any(|(start, _)| {
                    matching::recognized_credential_match(provider, value, "", start).is_some()
                })
            }),
            Self::Configured(provider) => provider.find_credential_matches(value).next().is_some(),
        }
    }

    pub(super) fn recognizes_strictly_embedded_dummy_collision_in(&self, value: &str) -> bool {
        match self {
            Self::Builtin(provider) => provider.credential_prefixes.iter().any(|prefix| {
                value.match_indices(*prefix).any(|(start, _)| {
                    matching::recognized_credential_match(provider, value, "", start)
                        .is_some_and(|credential| start > 0 || credential.len() < value.len())
                })
            }),
            Self::Configured(provider) => provider.contains_strictly_embedded_pattern_match(value),
        }
    }

    pub(super) fn contains_embedded_value(&self, text: &str, value: &str) -> bool {
        match self {
            Self::Builtin(_) => {
                value.len() >= MIN_EMBEDDED_CREDENTIAL_LENGTH && text.contains(value)
            }
            Self::Configured(provider) => provider
                .credential_value_match_ranges(text, value)
                .into_iter()
                .any(|range| {
                    value.len() >= MIN_EMBEDDED_CREDENTIAL_LENGTH
                        || !is_operational_path_match(text, range.start, range.end)
                }),
        }
    }

    pub(super) fn request_header_value(&self, value: &str) -> Option<HeaderValue> {
        match self {
            Self::Builtin(provider) => provider.request_header_value(value),
            Self::Configured(provider) => provider
                .matches_value(value)
                .then(|| provider.request_header_value(value))
                .flatten(),
        }
    }

    pub(super) fn translate_request_headers(
        &self,
        headers: &HeaderMap,
        expected_value: &str,
        replacement_value: &str,
    ) -> Vec<(HeaderName, HeaderValue)> {
        match self {
            Self::Builtin(provider) => provider
                .translate_request_header(headers, expected_value, replacement_value)
                .map(|value| (AUTHORIZATION, value)),
            Self::Configured(provider) => {
                return provider.translate_request_headers(
                    headers,
                    expected_value,
                    replacement_value,
                );
            }
        }
        .into_iter()
        .collect()
    }

    pub(super) fn insert_request_header(
        &self,
        headers: &mut HeaderMap,
        name: HeaderName,
        value: HeaderValue,
    ) {
        match self {
            Self::Builtin(provider) => provider.insert_request_header(headers, value),
            Self::Configured(_) => {
                headers.insert(name, value);
            }
        }
    }
}

impl CredentialRecord {
    pub(super) fn contains_embedded_value(&self, text: &str, value: &str) -> bool {
        !self.value_match_ranges(text, value).is_empty()
    }

    pub(super) fn value_match_ranges(
        &self,
        text: &str,
        value: &str,
    ) -> Vec<std::ops::Range<usize>> {
        let mut ranges = if text == value {
            std::iter::once(0..text.len()).collect()
        } else if is_builtin_shaped_credential(&self.real_value)
            || matches!(self.provider, BrokeredCredentialProvider::Builtin(_))
                && value.len() >= MIN_EMBEDDED_CREDENTIAL_LENGTH
        {
            text.match_indices(value)
                .map(|(start, _)| start..start + value.len())
                .collect()
        } else if let BrokeredCredentialProvider::Configured(provider) = &self.provider {
            provider
                .credential_value_match_ranges(text, value)
                .into_iter()
                .filter(|range| {
                    value.len() >= MIN_EMBEDDED_CREDENTIAL_LENGTH
                        || !is_operational_path_match(text, range.start, range.end)
                })
                .collect()
        } else {
            Vec::new()
        };
        if value == self.dummy_value {
            ranges.extend(self.generated_dummy_ranges(text));
            ranges.sort_by_key(|range| range.start);
            ranges.dedup();
        }
        ranges
    }
}

pub(super) fn is_builtin_shaped_credential(value: &str) -> bool {
    providers::credential_providers().any(|provider| {
        value.len() >= provider.minimum_credential_len
            && provider
                .credential_prefixes
                .iter()
                .any(|prefix| value.starts_with(prefix))
            && matching::recognized_credential_match(provider, value, "", /*start*/ 0)
                .is_some_and(|credential| credential == value)
    })
}

pub(super) fn select_credentials<'a>(
    headers: &HeaderMap,
    destination: &str,
    credentials: &'a [CredentialRecord],
    environment_id: Option<&str>,
) -> Vec<(&'a CredentialRecord, HeaderName, HeaderValue)> {
    let request = if destination.contains("://") {
        let Ok(request) = Url::parse(destination) else {
            return Vec::new();
        };
        Some(request)
    } else {
        None
    };
    let host = normalize_host(
        request
            .as_ref()
            .and_then(Url::host_str)
            .unwrap_or(destination),
    );
    let mut translated_matches = Vec::<(&CredentialRecord, HeaderName, HeaderValue)>::new();
    for credential in credentials.iter().filter(|credential| {
        credential.belongs_to_environment(environment_id)
            && credential
                .host_bindings()
                .any(|binding| binding.matches_request(&host, request.as_ref()))
    }) {
        for candidate in credentials.iter().filter(|candidate| {
            candidate.belongs_to_environment(environment_id)
                && candidate.provider.same_provider(&credential.provider)
                && candidate.real_value == credential.real_value
                && candidate
                    .host_bindings()
                    .any(|binding| binding.matches_request(&host, request.as_ref()))
        }) {
            for (name, value) in credential.provider.translate_request_headers(
                headers,
                &candidate.dummy_value,
                &credential.real_value,
            ) {
                if !translated_matches
                    .iter()
                    .any(|(existing, header, translated)| {
                        existing.provider.same_provider(&credential.provider)
                            && existing.real_value == credential.real_value
                            && *header == name
                            && *translated == value
                    })
                {
                    translated_matches.push((credential, name, value));
                }
            }
        }
    }
    let mut selected = Vec::<(&CredentialRecord, HeaderName, HeaderValue)>::new();
    let mut ambiguous_headers = Vec::<HeaderName>::new();
    for (credential, header_name, header_value) in translated_matches {
        if ambiguous_headers.contains(&header_name) {
            continue;
        }
        let Some(existing) = selected
            .iter()
            .position(|(_, selected_header, _)| selected_header == header_name)
        else {
            selected.push((credential, header_name, header_value));
            continue;
        };
        if !credential
            .provider
            .same_provider(&selected[existing].0.provider)
            || credential.real_value != selected[existing].0.real_value
            || header_value != selected[existing].2
        {
            if header_name == AUTHORIZATION
                && let Some(merged) = headers.get(AUTHORIZATION).and_then(|original| {
                    merge_basic_auth_fields(original, &selected[existing].2, &header_value)
                })
            {
                // Keep configured-provider raw-path validation for either field.
                if matches!(
                    credential.provider,
                    BrokeredCredentialProvider::Configured(_)
                ) {
                    selected[existing].0 = credential;
                }
                selected[existing].2 = merged;
                continue;
            }
            selected.swap_remove(existing);
            ambiguous_headers.push(header_name);
        }
    }
    if selected.iter().any(|(credential, _, _)| {
        matches!(
            &credential.provider,
            BrokeredCredentialProvider::Configured(_)
        )
    }) && let Some(request) = request.as_ref()
    {
        let Ok(raw_request) = destination.parse::<rama_http::Uri>() else {
            return Vec::new();
        };
        let raw_path = raw_request.path();
        if raw_path != request.path()
            || !crate::authorization_path::is_safe_for_authorization(raw_path)
        {
            return Vec::new();
        }
    }
    selected
}

fn merge_basic_auth_fields(
    original: &HeaderValue,
    left: &HeaderValue,
    right: &HeaderValue,
) -> Option<HeaderValue> {
    let decode = |header: &HeaderValue| {
        let (scheme, value) = header.to_str().ok()?.split_once(' ')?;
        scheme.eq_ignore_ascii_case("basic").then_some(())?;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(value.trim())
            .ok()?;
        let separator = decoded.iter().position(|byte| *byte == b':')?;
        Some((
            decoded[..separator].to_vec(),
            decoded[separator + 1..].to_vec(),
        ))
    };
    let (original_user, original_password) = decode(original)?;
    let (left_user, left_password) = decode(left)?;
    let (right_user, right_password) = decode(right)?;
    // Only combine independent fields. Overlapping translations remain ambiguous.
    let (user, password) = if left_user != original_user
        && left_password == original_password
        && right_user == original_user
        && right_password != original_password
    {
        (left_user, right_password)
    } else if right_user != original_user
        && right_password == original_password
        && left_user == original_user
        && left_password != original_password
    {
        (right_user, left_password)
    } else {
        return None;
    };
    let encoded =
        base64::engine::general_purpose::STANDARD.encode([user, vec![b':'], password].concat());
    HeaderValue::from_str(&format!("Basic {encoded}")).ok()
}

pub(super) fn active_credential_sources(
    state: &CredentialBrokerState,
    env: &HashMap<String, String>,
) -> Vec<ActiveCredentialSource> {
    let binding_env = state.context.with_fallbacks(env);
    let mut sources = Vec::new();
    for provider in providers::credential_providers() {
        for source in provider.sources() {
            if let Some(host_binding) =
                (source.host_binding)(&binding_env, state.openai_api_host.as_deref())
            {
                sources.push(ActiveCredentialSource {
                    provider: BrokeredCredentialProvider::Builtin(provider),
                    host_binding,
                    env_vars: source
                        .env_vars
                        .iter()
                        .map(|key| (*key).to_string())
                        .collect(),
                });
            }
        }
    }
    for provider in &state.configured_providers {
        if let Some(host_binding) = provider.host_binding(&binding_env) {
            sources.push(ActiveCredentialSource {
                provider: BrokeredCredentialProvider::Configured(provider.clone()),
                host_binding,
                env_vars: provider.config.env.clone(),
            });
        }
    }
    sources
}

pub(super) fn registered_credential_source(
    credential: &CredentialRecord,
    sources: &[ActiveCredentialSource],
    env: &HashMap<String, String>,
) -> Option<ActiveCredentialSource> {
    let retained_env_vars = match &credential.provider {
        BrokeredCredentialProvider::Builtin(provider) => provider
            .sources()
            .iter()
            .find(|source| {
                source
                    .env_vars
                    .iter()
                    .any(|key| env_key_matches(key, &credential.env_var))
                    && !source.binding_env_vars.is_empty()
                    && source
                        .binding_env_vars
                        .iter()
                        .all(|key| env_value(env, key).is_none())
            })
            .map(|source| {
                source
                    .env_vars
                    .iter()
                    .map(|key| (*key).to_string())
                    .collect()
            }),
        BrokeredCredentialProvider::Configured(provider) => provider
            .config
            .url_prefix_from_env
            .as_deref()
            .filter(|key| env_value(env, key).is_none())
            .map(|_| provider.config.env.clone()),
    };
    // Filtering a destination hint out of the child must not undo an existing
    // registration or rebind it to the broker's ambient fallback.
    if let Some(env_vars) = retained_env_vars {
        Some(ActiveCredentialSource {
            provider: credential.provider.clone(),
            host_binding: credential.host_binding.clone(),
            env_vars,
        })
    } else {
        sources
            .iter()
            .find(|source| source_accepts_credential(source, credential))
            .cloned()
            .map(|mut source| {
                // A new canonical token does not authorize an older alias at its destination.
                if !credential.invalidates_host_binding(env)
                    && !source_tracks_credential(&source, credential, env)
                {
                    source.host_binding = credential.host_binding.clone();
                }
                source
            })
    }
}

pub(super) fn prioritized_credentials<'a>(
    state: &'a CredentialBrokerState,
    env: &HashMap<String, String>,
) -> Vec<&'a CredentialRecord> {
    let binding_env = state.context.with_fallbacks(env);
    let mut credentials = state.credentials.iter().collect::<Vec<_>>();
    credentials.sort_unstable_by_key(|credential| {
        let active = env_entry(env, &credential.env_var)
            .is_some_and(|(_, value)| value == credential.dummy_value);
        let has_matching_host_binding = match &credential.provider {
            BrokeredCredentialProvider::Builtin(provider) => {
                provider.sources().iter().any(|source| {
                    !source.binding_env_vars.is_empty()
                        && (source.host_binding)(&binding_env, state.openai_api_host.as_deref())
                            .is_some_and(|binding| binding == credential.host_binding)
                })
            }
            BrokeredCredentialProvider::Configured(provider) => provider
                .config
                .url_prefix_from_env
                .as_deref()
                .is_some_and(|key| {
                    env_value(&binding_env, key).is_some()
                        && provider
                            .host_binding(&binding_env)
                            .is_some_and(|binding| binding == credential.host_binding)
                }),
        };
        (
            std::cmp::Reverse(credential.real_value.len()),
            std::cmp::Reverse(active),
            std::cmp::Reverse(has_matching_host_binding),
        )
    });
    credentials
}
