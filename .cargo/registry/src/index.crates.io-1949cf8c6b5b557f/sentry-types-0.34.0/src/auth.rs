use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::form_urlencoded;

use crate::dsn::Dsn;
use crate::protocol;
use crate::utils::{datetime_to_timestamp, timestamp_to_datetime};

/// Represents an auth header parsing error.
#[derive(Debug, Error, Copy, Clone, Eq, PartialEq)]
pub enum ParseAuthError {
    /// Raised if the auth header is not indicating sentry auth
    #[error("non sentry auth")]
    NonSentryAuth,
    /// Raised if the version value is invalid
    #[error("invalid value for version")]
    InvalidVersion,
    /// Raised if the public key is missing entirely
    #[error("missing public key in auth header")]
    MissingPublicKey,
}

/// Represents an auth header.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Auth {
    #[serde(skip)]
    timestamp: Option<SystemTime>,
    #[serde(rename = "sentry_client")]
    client: Option<String>,
    #[serde(rename = "sentry_version")]
    version: u16,
    #[serde(rename = "sentry_key")]
    key: String,
    #[serde(rename = "sentry_secret")]
    secret: Option<String>,
}

impl Auth {
    /// Creates an auth header from key value pairs.
    pub fn from_pairs<'a, I, K, V>(pairs: I) -> Result<Auth, ParseAuthError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: Into<Cow<'a, str>>,
    {
        let mut rv = Auth {
            timestamp: None,
            client: None,
            version: protocol::LATEST,
            key: "".into(),
            secret: None,
        };

        for (key, value) in pairs {
            let value = value.into();
            match key.as_ref() {
                "sentry_timestamp" => {
                    let timestamp = value.parse().ok().and_then(timestamp_to_datetime);

                    rv.timestamp = timestamp;
                }
                "sentry_client" => {
                    rv.client = Some(value.into());
                }
                "sentry_version" => {
                    rv.version = value
                        .split('.')
                        .next()
                        .and_then(|v| v.parse().ok())
                        .ok_or(ParseAuthError::InvalidVersion)?;
                }
                "sentry_key" => {
                    rv.key = value.into();
                }
                "sentry_secret" => {
                    rv.secret = Some(value.into());
                }
                _ => {}
            }
        }

        if rv.key.is_empty() {
            return Err(ParseAuthError::MissingPublicKey);
        }

        Ok(rv)
    }

    /// Creates an auth header from a query string.
    pub fn from_querystring(qs: &[u8]) -> Result<Auth, ParseAuthError> {
        Auth::from_pairs(form_urlencoded::parse(qs))
    }

    /// Returns the timestamp the client defined
    pub fn timestamp(&self) -> Option<SystemTime> {
        self.timestamp
    }

    /// Returns the protocol version the client speaks
    pub fn version(&self) -> u16 {
        self.version
    }

    /// Returns the public key
    pub fn public_key(&self) -> &str {
        &self.key
    }

    /// Returns the client's secret if it authenticated with a secret.
    pub fn secret_key(&self) -> Option<&str> {
        self.secret.as_deref()
    }

    /// Returns true if the authentication implies public auth (no secret)
    pub fn is_public(&self) -> bool {
        self.secret.is_none()
    }

    /// Returns the client's agent
    pub fn client_agent(&self) -> Option<&str> {
        self.client.as_deref()
    }
}

impl fmt::Display for Auth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Sentry sentry_key={}, sentry_version={}",
            self.key, self.version
        )?;
        if let Some(ts) = self.timestamp {
            write!(f, ", sentry_timestamp={}", datetime_to_timestamp(&ts))?;
        }
        if let Some(ref client) = self.client {
            write!(f, ", sentry_client={client}")?;
        }
        if let Some(ref secret) = self.secret {
            write!(f, ", sentry_secret={secret}")?;
        }
        Ok(())
    }
}

impl FromStr for Auth {
    type Err = ParseAuthError;

    fn from_str(s: &str) -> Result<Auth, ParseAuthError> {
        let mut base_iter = s.splitn(2, ' ');

        let prefix = base_iter.next().unwrap_or("");
        let items = base_iter.next().unwrap_or("");

        if !prefix.eq_ignore_ascii_case("sentry") {
            return Err(ParseAuthError::NonSentryAuth);
        }

        let auth = Self::from_pairs(items.split(',').filter_map(|item| {
            let mut kviter = item.split('=');
            Some((kviter.next()?.trim(), kviter.next()?.trim()))
        }))?;

        if auth.key.is_empty() {
            return Err(ParseAuthError::MissingPublicKey);
        }

        Ok(auth)
    }
}

pub(crate) fn auth_from_dsn_and_client(dsn: &Dsn, client: Option<&str>) -> Auth {
    Auth {
        timestamp: Some(SystemTime::now()),
        client: client.map(|x| x.to_string()),
        version: protocol::LATEST,
        key: dsn.public_key().to_string(),
        secret: dsn.secret_key().map(|x| x.to_string()),
    }
}
