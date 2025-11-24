use std::fmt;
use std::str::FromStr;

use thiserror::Error;
use url::Url;

use crate::auth::{auth_from_dsn_and_client, Auth};
use crate::project_id::{ParseProjectIdError, ProjectId};

/// Represents a dsn url parsing error.
#[derive(Debug, Error)]
pub enum ParseDsnError {
    /// raised on completely invalid urls
    #[error("no valid url provided")]
    InvalidUrl,
    /// raised the scheme is invalid / unsupported.
    #[error("no valid scheme")]
    InvalidScheme,
    /// raised if the username (public key) portion is missing.
    #[error("username is empty")]
    NoUsername,
    /// raised the project is is missing (first path component)
    #[error("empty path")]
    NoProjectId,
    /// raised the project id is invalid.
    #[error("invalid project id")]
    InvalidProjectId(#[from] ParseProjectIdError),
}

/// Represents the scheme of an url http/https.
///
/// This holds schemes that are supported by sentry and relays.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Scheme {
    /// unencrypted HTTP scheme (should not be used)
    Http,
    /// encrypted HTTPS scheme
    Https,
}

impl Scheme {
    /// Returns the default port for this scheme.
    pub fn default_port(self) -> u16 {
        match self {
            Scheme::Http => 80,
            Scheme::Https => 443,
        }
    }
}

impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Scheme::Https => "https",
                Scheme::Http => "http",
            }
        )
    }
}

/// Represents a Sentry dsn.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Dsn {
    scheme: Scheme,
    public_key: String,
    secret_key: Option<String>,
    host: String,
    port: Option<u16>,
    path: String,
    project_id: ProjectId,
}

impl Dsn {
    /// Converts the dsn into an auth object.
    ///
    /// This always attaches the latest and greatest protocol
    /// version to the auth header.
    pub fn to_auth(&self, client_agent: Option<&str>) -> Auth {
        auth_from_dsn_and_client(self, client_agent)
    }

    fn api_url(&self, endpoint: &str) -> Url {
        use std::fmt::Write;
        let mut buf = format!("{}://{}", self.scheme(), self.host());
        if self.port() != self.scheme.default_port() {
            write!(&mut buf, ":{}", self.port()).unwrap();
        }
        write!(
            &mut buf,
            "{}api/{}/{}/",
            self.path,
            self.project_id(),
            endpoint
        )
        .unwrap();
        Url::parse(&buf).unwrap()
    }

    /// Returns the submission API URL.
    pub fn store_api_url(&self) -> Url {
        self.api_url("store")
    }

    /// Returns the API URL for Envelope submission.
    pub fn envelope_api_url(&self) -> Url {
        self.api_url("envelope")
    }

    /// Returns the scheme
    pub fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// Returns the public_key
    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    /// Returns secret_key
    pub fn secret_key(&self) -> Option<&str> {
        self.secret_key.as_deref()
    }

    /// Returns the host
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Returns the port
    pub fn port(&self) -> u16 {
        self.port.unwrap_or_else(|| self.scheme.default_port())
    }

    /// Returns the path
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the project_id
    pub fn project_id(&self) -> &ProjectId {
        &self.project_id
    }
}

impl fmt::Display for Dsn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}://{}:", self.scheme, self.public_key)?;
        if let Some(ref secret_key) = self.secret_key {
            write!(f, "{secret_key}")?;
        }
        write!(f, "@{}", self.host)?;
        if let Some(ref port) = self.port {
            write!(f, ":{port}")?;
        }
        write!(f, "{}{}", self.path, self.project_id)?;
        Ok(())
    }
}

impl FromStr for Dsn {
    type Err = ParseDsnError;

    fn from_str(s: &str) -> Result<Dsn, ParseDsnError> {
        let url = Url::parse(s).map_err(|_| ParseDsnError::InvalidUrl)?;

        if url.path() == "/" {
            return Err(ParseDsnError::NoProjectId);
        }

        let mut path_segments = url.path().trim_matches('/').rsplitn(2, '/');

        let project_id = path_segments
            .next()
            .ok_or(ParseDsnError::NoProjectId)?
            .parse()
            .map_err(ParseDsnError::InvalidProjectId)?;
        let path = match path_segments.next().unwrap_or("") {
            "" | "/" => "/".into(),
            other => format!("/{other}/"),
        };

        let public_key = match url.username() {
            "" => return Err(ParseDsnError::NoUsername),
            username => username.to_string(),
        };

        let scheme = match url.scheme() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            _ => return Err(ParseDsnError::InvalidScheme),
        };

        let secret_key = url.password().map(|s| s.into());
        let port = url.port();
        let host = match url.host_str() {
            Some(host) => host.into(),
            None => return Err(ParseDsnError::InvalidUrl),
        };

        Ok(Dsn {
            scheme,
            public_key,
            secret_key,
            host,
            port,
            path,
            project_id,
        })
    }
}

impl_str_serde!(Dsn);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_dsn_serialize_deserialize() {
        let dsn = Dsn::from_str("https://username:@domain/42").unwrap();
        let serialized = serde_json::to_string(&dsn).unwrap();
        assert_eq!(serialized, "\"https://username:@domain/42\"");
        let deserialized: Dsn = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.to_string(), "https://username:@domain/42");
    }

    #[test]
    fn test_dsn_parsing() {
        let url = "https://username:password@domain:8888/23%21";
        let dsn = url.parse::<Dsn>().unwrap();
        assert_eq!(dsn.scheme(), Scheme::Https);
        assert_eq!(dsn.public_key(), "username");
        assert_eq!(dsn.secret_key(), Some("password"));
        assert_eq!(dsn.host(), "domain");
        assert_eq!(dsn.port(), 8888);
        assert_eq!(dsn.path(), "/");
        assert_eq!(dsn.project_id(), &ProjectId::new("23%21"));
        assert_eq!(url, dsn.to_string());
    }

    #[test]
    fn test_dsn_no_port() {
        let url = "https://username:@domain/42";
        let dsn = Dsn::from_str(url).unwrap();
        assert_eq!(dsn.port(), 443);
        assert_eq!(url, dsn.to_string());
        assert_eq!(
            dsn.store_api_url().to_string(),
            "https://domain/api/42/store/"
        );
        assert_eq!(
            dsn.envelope_api_url().to_string(),
            "https://domain/api/42/envelope/"
        );
    }

    #[test]
    fn test_insecure_dsn_no_port() {
        let url = "http://username:@domain/42";
        let dsn = Dsn::from_str(url).unwrap();
        assert_eq!(dsn.port(), 80);
        assert_eq!(url, dsn.to_string());
        assert_eq!(
            dsn.store_api_url().to_string(),
            "http://domain/api/42/store/"
        );
        assert_eq!(
            dsn.envelope_api_url().to_string(),
            "http://domain/api/42/envelope/"
        );
    }

    #[test]
    fn test_dsn_no_password() {
        let url = "https://username:@domain:8888/42";
        let dsn = Dsn::from_str(url).unwrap();
        assert_eq!(url, dsn.to_string());
        assert_eq!(
            dsn.store_api_url().to_string(),
            "https://domain:8888/api/42/store/"
        );
        assert_eq!(
            dsn.envelope_api_url().to_string(),
            "https://domain:8888/api/42/envelope/"
        );
    }

    #[test]
    fn test_dsn_no_password_colon() {
        let url = "https://username@domain:8888/42";
        let dsn = Dsn::from_str(url).unwrap();
        assert_eq!("https://username:@domain:8888/42", dsn.to_string());
    }

    #[test]
    fn test_dsn_http_url() {
        let url = "http://username:@domain:8888/42";
        let dsn = Dsn::from_str(url).unwrap();
        assert_eq!(url, dsn.to_string());
    }

    #[test]
    fn test_dsn_non_integer_project_id() {
        let url = "https://username:password@domain:8888/abc123youandme%21%21";
        let dsn = url.parse::<Dsn>().unwrap();
        assert_eq!(dsn.project_id(), &ProjectId::new("abc123youandme%21%21"));
    }

    #[test]
    fn test_dsn_more_than_one_non_integer_path() {
        let url = "http://username:@domain:8888/pathone/pathtwo/pid";
        let dsn = url.parse::<Dsn>().unwrap();
        assert_eq!(dsn.project_id(), &ProjectId::new("pid"));
        assert_eq!(dsn.path(), "/pathone/pathtwo/");
    }

    #[test]
    #[should_panic(expected = "NoUsername")]
    fn test_dsn_no_username() {
        Dsn::from_str("https://:password@domain:8888/23").unwrap();
    }

    #[test]
    #[should_panic(expected = "InvalidUrl")]
    fn test_dsn_invalid_url() {
        Dsn::from_str("random string").unwrap();
    }

    #[test]
    #[should_panic(expected = "InvalidUrl")]
    fn test_dsn_no_host() {
        Dsn::from_str("https://username:password@:8888/42").unwrap();
    }

    #[test]
    #[should_panic(expected = "NoProjectId")]
    fn test_dsn_no_project_id() {
        Dsn::from_str("https://username:password@domain:8888/").unwrap();
    }

    #[test]
    #[should_panic(expected = "InvalidScheme")]
    fn test_dsn_invalid_scheme() {
        Dsn::from_str("ftp://username:password@domain:8888/1").unwrap();
    }
}
