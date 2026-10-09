use crate::authorization_path::is_safe_for_authorization;
use crate::policy::Host;
use crate::policy::is_loopback_host;
use crate::policy::normalize_host;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use url::Url;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct CredentialDestination {
    scheme: CredentialDestinationScheme,
    host: String,
    wildcard: bool,
    port: u16,
    path_prefix: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CredentialDestinationScheme {
    Http,
    Https,
}

impl CredentialDestinationScheme {
    fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }

    fn default_port(self) -> u16 {
        match self {
            Self::Http => 80,
            Self::Https => 443,
        }
    }
}

impl CredentialDestination {
    pub(super) fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        let (explicit_scheme, authority) = value
            .split_once("://")
            .map_or((None, value), |(scheme, authority)| {
                (Some(scheme), authority)
            });
        let (wildcard, authority) = authority
            .strip_prefix("*.")
            .map_or((false, authority), |suffix| (true, suffix));
        let authority = if wildcard {
            format!("wildcard.{authority}")
        } else {
            authority.to_string()
        };
        let url = Url::parse(&format!("https://{authority}"))?;
        ensure!(
            url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "credential destination cannot include user information, a query, or a fragment"
        );
        let host = url
            .host_str()
            .context("credential destination has no hostname")?;
        let host = if wildcard {
            host.strip_prefix("wildcard.")
                .context("credential destination has an invalid wildcard")?
        } else {
            host
        };
        let host = normalize_host(host);
        ensure!(
            !host.is_empty() && !host.contains('*') && !host.contains('/'),
            "credential destination must have an exact hostname or scoped wildcard"
        );
        let loopback = !wildcard && is_loopback_host(&Host::parse(&host)?);
        let scheme = match explicit_scheme {
            None if loopback => CredentialDestinationScheme::Http,
            None => CredentialDestinationScheme::Https,
            Some(scheme) if scheme.eq_ignore_ascii_case("https") => {
                CredentialDestinationScheme::Https
            }
            Some(scheme) if scheme.eq_ignore_ascii_case("http") && loopback => {
                CredentialDestinationScheme::Http
            }
            Some(_) => {
                bail!("credential destination must use HTTPS unless it targets loopback over HTTP")
            }
        };
        // HTTPS parsing removes an explicit :443, which HTTP must retain.
        let url = match scheme {
            CredentialDestinationScheme::Http => Url::parse(&format!("http://{authority}"))?,
            CredentialDestinationScheme::Https => url,
        };

        Ok(Self {
            scheme,
            host,
            wildcard,
            port: url.port().unwrap_or_else(|| scheme.default_port()),
            path_prefix: url.path().to_string(),
        })
    }

    pub(super) fn is_wildcard(&self) -> bool {
        self.wildcard
    }

    pub(super) fn bypasses_proxy_with_local_binding(&self) -> bool {
        (!self.wildcard || self.host.parse::<std::net::IpAddr>().is_err())
            && crate::policy::is_default_proxy_bypass_host(&self.host)
    }

    pub(super) fn matches_host(&self, host: &str, port: u16) -> bool {
        self.port == port && self.matches_hostname(host)
    }

    pub(super) fn matches_hostname(&self, host: &str) -> bool {
        if self.wildcard {
            host.strip_suffix(&self.host)
                .is_some_and(|prefix| prefix.ends_with('.'))
        } else {
            host == self.host
        }
    }

    pub(super) fn requires_mitm(&self, host: &str, port: u16) -> bool {
        self.scheme == CredentialDestinationScheme::Https && self.matches_host(host, port)
    }

    pub(super) fn requires_http_interception(&self, host: &str, port: u16) -> bool {
        self.scheme == CredentialDestinationScheme::Http && self.matches_host(host, port)
    }

    pub(super) fn matches_request(&self, host: &str, request: Option<&Url>) -> bool {
        let Some(request) = request else {
            return false;
        };
        if request.scheme() != self.scheme.as_str()
            || !self.matches_host(
                host,
                request
                    .port_or_known_default()
                    .unwrap_or_else(|| self.scheme.default_port()),
            )
        {
            return false;
        }
        let path = request.path();
        if !is_safe_for_authorization(path) {
            return false;
        }
        self.path_prefix == "/"
            || path == self.path_prefix
            || path
                .strip_prefix(&self.path_prefix)
                .is_some_and(|suffix| self.path_prefix.ends_with('/') || suffix.starts_with('/'))
    }
}
