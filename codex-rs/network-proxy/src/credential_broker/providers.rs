mod github;
mod openai;

use super::destination::CredentialDestination;
use rama_http::HeaderMap;
use rama_http::HeaderValue;
use rand::Rng as _;
use std::collections::HashMap;
use url::Url;

const DUMMY_ALPHANUMERIC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

type TranslateRequestHeader = fn(&HeaderMap, &str, &str) -> Option<HeaderValue>;

/// Describes how one credential family is recognized and injected.
///
/// Providers must be declared as `static` values because the broker uses their addresses as stable
/// identities when deduplicating credential records.
pub(super) struct CredentialProvider {
    pub(super) context_env_vars: &'static [&'static str],
    pub(super) credential_prefixes: &'static [&'static str],
    pub(super) ignored_credential_prefixes: &'static [&'static str],
    pub(super) credential_watermark: Option<&'static str>,
    pub(super) minimum_credential_len: usize,
    sources: &'static [CredentialSource],
    pub(super) reset_on_configuration_change: bool,
    dummy_value: fn(&str) -> String,
    translate_request_header: TranslateRequestHeader,
    request_header_value: fn(&str) -> Option<HeaderValue>,
    insert_request_header: fn(&mut HeaderMap, HeaderValue),
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum CredentialHostBinding {
    ExactHost(String),
    ExactHosts(Vec<String>),
    HostPattern {
        exact_hosts: &'static [&'static str],
        suffixes: &'static [&'static str],
    },
    ConfiguredHosts(Vec<CredentialDestination>),
}

pub(super) struct CredentialSource {
    pub(super) env_vars: &'static [&'static str],
    pub(super) binding_env_vars: &'static [&'static str],
    // An explicit invalid hint can clear dynamic destinations while leaving a static fallback.
    pub(super) invalidates_host_binding: fn(&HashMap<String, String>) -> bool,
    pub(super) host_binding:
        fn(&HashMap<String, String>, Option<&str>) -> Option<CredentialHostBinding>,
}

const CREDENTIAL_PROVIDERS: &[&CredentialProvider] = &[&github::PROVIDER, &openai::PROVIDER];

impl CredentialProvider {
    pub(super) fn sources(&self) -> &[CredentialSource] {
        self.sources
    }

    pub(super) fn dummy_value(&self, real_value: &str) -> String {
        (self.dummy_value)(real_value)
    }

    pub(super) fn translate_request_header(
        &self,
        headers: &HeaderMap,
        expected_value: &str,
        replacement_value: &str,
    ) -> Option<HeaderValue> {
        (self.translate_request_header)(headers, expected_value, replacement_value)
    }

    pub(super) fn request_header_value(&self, value: &str) -> Option<HeaderValue> {
        (self.request_header_value)(value)
    }

    pub(super) fn insert_request_header(&self, headers: &mut HeaderMap, value: HeaderValue) {
        (self.insert_request_header)(headers, value);
    }
}

impl CredentialHostBinding {
    pub(super) fn bypasses_proxy_with_local_binding(&self) -> bool {
        use crate::policy::is_default_proxy_bypass_host;

        match self {
            Self::ExactHost(host) => is_default_proxy_bypass_host(host),
            Self::ExactHosts(hosts) => hosts.iter().any(|host| is_default_proxy_bypass_host(host)),
            Self::HostPattern { exact_hosts, .. } => exact_hosts
                .iter()
                .any(|host| is_default_proxy_bypass_host(host)),
            Self::ConfiguredHosts(destinations) => destinations
                .iter()
                .any(CredentialDestination::bypasses_proxy_with_local_binding),
        }
    }

    pub(super) fn matches_host(&self, host: &str, port: u16) -> bool {
        match self {
            Self::ExactHost(expected_host) => host == expected_host,
            Self::ExactHosts(expected_hosts) => {
                expected_hosts.iter().any(|expected| host == expected)
            }
            Self::HostPattern {
                exact_hosts,
                suffixes,
            } => {
                exact_hosts.contains(&host) || suffixes.iter().any(|suffix| host.ends_with(suffix))
            }
            Self::ConfiguredHosts(destinations) => destinations
                .iter()
                .any(|destination| destination.matches_host(host, port)),
        }
    }

    // Hooks apply to the host regardless of each credential destination's port or path.
    pub(super) fn matches_hostname(&self, host: &str) -> bool {
        match self {
            Self::ConfiguredHosts(destinations) => destinations
                .iter()
                .any(|destination| destination.matches_hostname(host)),
            Self::ExactHost(_) | Self::ExactHosts(_) | Self::HostPattern { .. } => {
                self.matches_host(host, /*port*/ 443)
            }
        }
    }

    pub(super) fn requires_mitm(&self, host: &str, port: u16) -> bool {
        match self {
            Self::ConfiguredHosts(destinations) => destinations
                .iter()
                .any(|destination| destination.requires_mitm(host, port)),
            Self::ExactHost(_) | Self::ExactHosts(_) | Self::HostPattern { .. } => {
                self.matches_host(host, port)
            }
        }
    }

    pub(super) fn matches_request(&self, host: &str, request: Option<&Url>) -> bool {
        match self {
            Self::ConfiguredHosts(destinations) => destinations
                .iter()
                .any(|destination| destination.matches_request(host, request)),
            Self::ExactHost(_) | Self::ExactHosts(_) | Self::HostPattern { .. } => {
                request.is_none_or(|request| request.scheme() == "https")
                    && self.matches_host(
                        host,
                        request.and_then(Url::port_or_known_default).unwrap_or(443),
                    )
            }
        }
    }
}

pub(super) fn credential_context_env_keys(
    brokered_keys: &[String],
) -> impl Iterator<Item = &'static str> + '_ {
    credential_providers()
        .filter(move |provider| {
            provider.sources().iter().any(|source| {
                source.env_vars.iter().any(|key| {
                    brokered_keys
                        .iter()
                        .any(|brokered_key| super::env_key_matches(brokered_key, key))
                })
            })
        })
        .flat_map(|provider| provider.context_env_vars.iter().copied())
}

pub(super) fn credential_binding_env_keys(
    brokered_keys: &[String],
) -> impl Iterator<Item = &'static str> + '_ {
    credential_providers()
        .flat_map(CredentialProvider::sources)
        .filter(move |source| {
            source.env_vars.iter().any(|key| {
                brokered_keys
                    .iter()
                    .any(|brokered_key| super::env_key_matches(brokered_key, key))
            })
        })
        .flat_map(|source| source.binding_env_vars.iter().copied())
}

pub(super) fn credential_env_keys() -> impl Iterator<Item = &'static str> {
    credential_providers()
        .flat_map(CredentialProvider::sources)
        .flat_map(|source| source.env_vars.iter().copied())
}

pub(super) fn credential_providers() -> impl Iterator<Item = &'static CredentialProvider> {
    CREDENTIAL_PROVIDERS.iter().copied()
}

pub(super) fn translate_standard_request_header(
    headers: &HeaderMap,
    expected_value: &str,
    replacement_value: &str,
) -> Option<HeaderValue> {
    github::translate_request_header(headers, expected_value, replacement_value)
}

fn shaped_dummy_value(real_value: &str, prefix: &str, minimum_len: usize) -> String {
    let target_len = real_value.len().max(minimum_len).max(prefix.len() + 16);
    let mut rng = rand::rng();
    let mut dummy = String::with_capacity(target_len);
    dummy.push_str(prefix);
    for index in prefix.len()..target_len {
        let character = match real_value.as_bytes().get(index).copied() {
            Some(template) if !template.is_ascii_alphanumeric() => template,
            _ => DUMMY_ALPHANUMERIC[rng.random_range(0..DUMMY_ALPHANUMERIC.len())],
        };
        dummy.push(char::from(character));
    }
    dummy
}
