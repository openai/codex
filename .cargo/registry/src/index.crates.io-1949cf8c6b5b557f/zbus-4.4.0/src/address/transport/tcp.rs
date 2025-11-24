use super::encode_percents;
use crate::{Error, Result};
#[cfg(not(feature = "tokio"))]
use async_io::Async;
#[cfg(not(feature = "tokio"))]
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    str::FromStr,
};
#[cfg(feature = "tokio")]
use tokio::net::TcpStream;

/// A TCP transport in a D-Bus address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tcp {
    pub(super) host: String,
    pub(super) bind: Option<String>,
    pub(super) port: u16,
    pub(super) family: Option<TcpTransportFamily>,
    pub(super) nonce_file: Option<Vec<u8>>,
}

impl Tcp {
    /// Create a new TCP transport with the given host and port.
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_owned(),
            port,
            bind: None,
            family: None,
            nonce_file: None,
        }
    }

    /// Set the `tcp:` address `bind` value.
    pub fn set_bind(mut self, bind: Option<String>) -> Self {
        self.bind = bind;

        self
    }

    /// Set the `tcp:` address `family` value.
    pub fn set_family(mut self, family: Option<TcpTransportFamily>) -> Self {
        self.family = family;

        self
    }

    /// Set the `tcp:` address `noncefile` value.
    pub fn set_nonce_file(mut self, nonce_file: Option<Vec<u8>>) -> Self {
        self.nonce_file = nonce_file;

        self
    }

    /// Returns the `tcp:` address `host` value.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Returns the `tcp:` address `bind` value.
    pub fn bind(&self) -> Option<&str> {
        self.bind.as_deref()
    }

    /// Returns the `tcp:` address `port` value.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns the `tcp:` address `family` value.
    pub fn family(&self) -> Option<TcpTransportFamily> {
        self.family
    }

    /// The nonce file path, if any.
    pub fn nonce_file(&self) -> Option<&[u8]> {
        self.nonce_file.as_deref()
    }

    /// Take ownership of the nonce file path, if any.
    pub fn take_nonce_file(&mut self) -> Option<Vec<u8>> {
        self.nonce_file.take()
    }

    pub(super) fn from_options(
        opts: HashMap<&str, &str>,
        nonce_tcp_required: bool,
    ) -> Result<Self> {
        let bind = None;
        if opts.contains_key("bind") {
            return Err(Error::Address("`bind` isn't yet supported".into()));
        }

        let host = opts
            .get("host")
            .ok_or_else(|| Error::Address("tcp address is missing `host`".into()))?
            .to_string();
        let port = opts
            .get("port")
            .ok_or_else(|| Error::Address("tcp address is missing `port`".into()))?;
        let port = port
            .parse::<u16>()
            .map_err(|_| Error::Address("invalid tcp `port`".into()))?;
        let family = opts
            .get("family")
            .map(|f| TcpTransportFamily::from_str(f))
            .transpose()?;
        let nonce_file = opts
            .get("noncefile")
            .map(|f| super::decode_percents(f))
            .transpose()?;
        if nonce_tcp_required && nonce_file.is_none() {
            return Err(Error::Address(
                "nonce-tcp address is missing `noncefile`".into(),
            ));
        }

        Ok(Self {
            host,
            bind,
            port,
            family,
            nonce_file,
        })
    }

    #[cfg(not(feature = "tokio"))]
    pub(super) async fn connect(self) -> Result<Async<TcpStream>> {
        let addrs = crate::Task::spawn_blocking(
            move || -> Result<Vec<SocketAddr>> {
                let addrs = (self.host(), self.port()).to_socket_addrs()?.filter(|a| {
                    if let Some(family) = self.family() {
                        if family == TcpTransportFamily::Ipv4 {
                            a.is_ipv4()
                        } else {
                            a.is_ipv6()
                        }
                    } else {
                        true
                    }
                });
                Ok(addrs.collect())
            },
            "connect tcp",
        )
        .await
        .map_err(|e| Error::Address(format!("Failed to receive TCP addresses: {e}")))?;

        // we could attempt connections in parallel?
        let mut last_err = Error::Address("Failed to connect".into());
        for addr in addrs {
            match Async::<TcpStream>::connect(addr).await {
                Ok(stream) => return Ok(stream),
                Err(e) => last_err = e.into(),
            }
        }

        Err(last_err)
    }

    #[cfg(feature = "tokio")]
    pub(super) async fn connect(self) -> Result<TcpStream> {
        TcpStream::connect((self.host(), self.port()))
            .await
            .map_err(|e| Error::InputOutput(e.into()))
    }
}

impl Display for Tcp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.nonce_file() {
            Some(nonce_file) => {
                f.write_str("nonce-tcp:noncefile=")?;
                encode_percents(f, nonce_file)?;
                f.write_str(",")?;
            }
            None => f.write_str("tcp:")?,
        }
        f.write_str("host=")?;

        encode_percents(f, self.host().as_bytes())?;

        write!(f, ",port={}", self.port())?;

        if let Some(bind) = self.bind() {
            f.write_str(",bind=")?;
            encode_percents(f, bind.as_bytes())?;
        }

        if let Some(family) = self.family() {
            write!(f, ",family={family}")?;
        }

        Ok(())
    }
}

/// A `tcp:` address family.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TcpTransportFamily {
    Ipv4,
    Ipv6,
}

impl FromStr for TcpTransportFamily {
    type Err = Error;

    fn from_str(family: &str) -> Result<Self> {
        match family {
            "ipv4" => Ok(Self::Ipv4),
            "ipv6" => Ok(Self::Ipv6),
            _ => Err(Error::Address(format!(
                "invalid tcp address `family`: {family}"
            ))),
        }
    }
}

impl Display for TcpTransportFamily {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ipv4 => write!(f, "ipv4"),
            Self::Ipv6 => write!(f, "ipv6"),
        }
    }
}
