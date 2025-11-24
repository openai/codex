use base64::{prelude::BASE64_STANDARD, Engine};

use crate::{
    error::{Error, ErrorKind},
    Response,
};

/// Proxy protocol
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Proto {
    HTTP,
    SOCKS4,
    SOCKS4A,
    SOCKS5,
}

/// Proxy server definition
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Proxy {
    pub(crate) server: String,
    pub(crate) port: u32,
    pub(crate) user: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) proto: Proto,
}

impl Proxy {
    fn parse_creds<S: AsRef<str>>(
        creds: &Option<S>,
    ) -> Result<(Option<String>, Option<String>), Error> {
        match creds {
            Some(creds) => {
                let mut parts = creds
                    .as_ref()
                    .splitn(2, ':')
                    .collect::<Vec<&str>>()
                    .into_iter();

                if parts.len() != 2 {
                    Err(ErrorKind::InvalidProxyUrl.new())
                } else {
                    Ok((
                        parts.next().map(String::from),
                        parts.next().map(String::from),
                    ))
                }
            }
            None => Ok((None, None)),
        }
    }

    fn parse_address<S: AsRef<str>>(host: &Option<S>) -> Result<(String, Option<u32>), Error> {
        match host {
            Some(host) => {
                let mut parts = host.as_ref().split(':').collect::<Vec<&str>>().into_iter();
                let host = parts
                    .next()
                    .ok_or_else(|| ErrorKind::InvalidProxyUrl.new())?;
                let port = parts.next();
                Ok((
                    String::from(host),
                    port.and_then(|port| port.parse::<u32>().ok()),
                ))
            }
            None => Err(ErrorKind::InvalidProxyUrl.new()),
        }
    }

    pub(crate) fn use_authorization(&self) -> bool {
        self.user.is_some() && self.password.is_some()
    }

    pub(crate) fn try_from_system() -> Option<Self> {
        macro_rules! try_env {
            ($($env:literal),+) => {
                $(
                    if let Ok(env) = std::env::var($env) {
                        if let Ok(proxy) = Self::new(env) {
                            return Some(proxy);
                        }
                    }
                )+
            };
        }

        try_env!(
            "ALL_PROXY",
            "all_proxy",
            "HTTPS_PROXY",
            "https_proxy",
            "HTTP_PROXY",
            "http_proxy"
        );
        None
    }

    /// Create a proxy from a format string.
    /// # Arguments:
    /// * `proxy` - a str of format `<protocol>://<user>:<password>@<host>:port` . All parts except host are optional.
    /// # Protocols
    /// * `http`: HTTP
    /// * `socks4`: SOCKS4 (requires socks feature)
    /// * `socks4a`: SOCKS4A (requires socks feature)
    /// * `socks5` and `socks`: SOCKS5 (requires socks feature)
    /// # Examples
    /// * `http://127.0.0.1:8080`
    /// * `socks5://john:smith@socks.google.com`
    /// * `john:smith@socks.google.com:8000`
    /// * `localhost`
    pub fn new<S: AsRef<str>>(proxy: S) -> Result<Self, Error> {
        let mut proxy = proxy.as_ref();

        while proxy.ends_with('/') {
            proxy = &proxy[..(proxy.len() - 1)];
        }

        let mut proxy_parts = proxy.splitn(2, "://").collect::<Vec<&str>>().into_iter();

        let proto = if proxy_parts.len() == 2 {
            match proxy_parts.next() {
                Some("http") => Proto::HTTP,
                Some("socks4") => Proto::SOCKS4,
                Some("socks4a") => Proto::SOCKS4A,
                Some("socks") => Proto::SOCKS5,
                Some("socks5") => Proto::SOCKS5,
                _ => return Err(ErrorKind::InvalidProxyUrl.new()),
            }
        } else {
            Proto::HTTP
        };

        let remaining_parts = proxy_parts.next();
        if remaining_parts.is_none() {
            return Err(ErrorKind::InvalidProxyUrl.new());
        }

        let mut creds_server_port_parts = remaining_parts
            .unwrap()
            .rsplitn(2, '@')
            .collect::<Vec<&str>>()
            .into_iter()
            .rev();

        let (user, password) = if creds_server_port_parts.len() == 2 {
            Proxy::parse_creds(&creds_server_port_parts.next())?
        } else {
            (None, None)
        };

        let (server, port) = Proxy::parse_address(&creds_server_port_parts.next())?;

        Ok(Self {
            server,
            user,
            password,
            port: port.unwrap_or(match proto {
                Proto::HTTP => 80,
                Proto::SOCKS4 | Proto::SOCKS4A | Proto::SOCKS5 => 1080,
            }),
            proto,
        })
    }

    pub(crate) fn connect<S: AsRef<str>>(&self, host: S, port: u16, user_agent: &str) -> String {
        let authorization = if self.use_authorization() {
            let creds = BASE64_STANDARD.encode(format!(
                "{}:{}",
                self.user.clone().unwrap_or_default(),
                self.password.clone().unwrap_or_default()
            ));

            match self.proto {
                Proto::HTTP => format!("Proxy-Authorization: basic {}\r\n", creds),
                _ => String::new(),
            }
        } else {
            String::new()
        };

        format!(
            "CONNECT {}:{} HTTP/1.1\r\n\
Host: {}:{}\r\n\
User-Agent: {}\r\n\
Proxy-Connection: Keep-Alive\r\n\
{}\
\r\n",
            host.as_ref(),
            port,
            host.as_ref(),
            port,
            user_agent,
            authorization
        )
    }

    pub(crate) fn verify_response(response: &Response) -> Result<(), Error> {
        match response.status() {
            200 => Ok(()),
            401 | 407 => Err(ErrorKind::ProxyUnauthorized.new()),
            _ => Err(ErrorKind::ProxyConnect.new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_proxy_fakeproto() {
        assert!(Proxy::new("fakeproto://localhost").is_err());
    }

    #[test]
    fn parse_proxy_http_user_pass_server_port() {
        let proxy = Proxy::new("http://user:p@ssw0rd@localhost:9999").unwrap();
        assert_eq!(proxy.user, Some(String::from("user")));
        assert_eq!(proxy.password, Some(String::from("p@ssw0rd")));
        assert_eq!(proxy.server, String::from("localhost"));
        assert_eq!(proxy.port, 9999);
        assert_eq!(proxy.proto, Proto::HTTP);
    }

    #[test]
    fn parse_proxy_http_user_pass_server_port_trailing_slash() {
        let proxy = Proxy::new("http://user:p@ssw0rd@localhost:9999/").unwrap();
        assert_eq!(proxy.user, Some(String::from("user")));
        assert_eq!(proxy.password, Some(String::from("p@ssw0rd")));
        assert_eq!(proxy.server, String::from("localhost"));
        assert_eq!(proxy.port, 9999);
        assert_eq!(proxy.proto, Proto::HTTP);
    }

    #[cfg(feature = "socks-proxy")]
    #[test]
    fn parse_proxy_socks4_user_pass_server_port() {
        let proxy = Proxy::new("socks4://user:p@ssw0rd@localhost:9999").unwrap();
        assert_eq!(proxy.user, Some(String::from("user")));
        assert_eq!(proxy.password, Some(String::from("p@ssw0rd")));
        assert_eq!(proxy.server, String::from("localhost"));
        assert_eq!(proxy.port, 9999);
        assert_eq!(proxy.proto, Proto::SOCKS4);
    }

    #[cfg(feature = "socks-proxy")]
    #[test]
    fn parse_proxy_socks4a_user_pass_server_port() {
        let proxy = Proxy::new("socks4a://user:p@ssw0rd@localhost:9999").unwrap();
        assert_eq!(proxy.user, Some(String::from("user")));
        assert_eq!(proxy.password, Some(String::from("p@ssw0rd")));
        assert_eq!(proxy.server, String::from("localhost"));
        assert_eq!(proxy.port, 9999);
        assert_eq!(proxy.proto, Proto::SOCKS4A);
    }

    #[cfg(feature = "socks-proxy")]
    #[test]
    fn parse_proxy_socks_user_pass_server_port() {
        let proxy = Proxy::new("socks://user:p@ssw0rd@localhost:9999").unwrap();
        assert_eq!(proxy.user, Some(String::from("user")));
        assert_eq!(proxy.password, Some(String::from("p@ssw0rd")));
        assert_eq!(proxy.server, String::from("localhost"));
        assert_eq!(proxy.port, 9999);
        assert_eq!(proxy.proto, Proto::SOCKS5);
    }

    #[cfg(feature = "socks-proxy")]
    #[test]
    fn parse_proxy_socks5_user_pass_server_port() {
        let proxy = Proxy::new("socks5://user:p@ssw0rd@localhost:9999").unwrap();
        assert_eq!(proxy.user, Some(String::from("user")));
        assert_eq!(proxy.password, Some(String::from("p@ssw0rd")));
        assert_eq!(proxy.server, String::from("localhost"));
        assert_eq!(proxy.port, 9999);
        assert_eq!(proxy.proto, Proto::SOCKS5);
    }

    #[test]
    fn parse_proxy_user_pass_server_port() {
        let proxy = Proxy::new("user:p@ssw0rd@localhost:9999").unwrap();
        assert_eq!(proxy.user, Some(String::from("user")));
        assert_eq!(proxy.password, Some(String::from("p@ssw0rd")));
        assert_eq!(proxy.server, String::from("localhost"));
        assert_eq!(proxy.port, 9999);
    }

    #[test]
    fn parse_proxy_server_port() {
        let proxy = Proxy::new("localhost:9999").unwrap();
        assert_eq!(proxy.user, None);
        assert_eq!(proxy.password, None);
        assert_eq!(proxy.server, String::from("localhost"));
        assert_eq!(proxy.port, 9999);
    }

    #[test]
    fn parse_proxy_server() {
        let proxy = Proxy::new("localhost").unwrap();
        assert_eq!(proxy.user, None);
        assert_eq!(proxy.password, None);
        assert_eq!(proxy.server, String::from("localhost"));
    }
}
