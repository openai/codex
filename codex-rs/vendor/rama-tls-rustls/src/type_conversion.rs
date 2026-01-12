use crate::{RamaFrom, RamaTryFrom};
use rama_core::error::{ErrorContext, OpaqueError};
use rama_net::{
    address::{Domain, Host},
    tls::{
        ApplicationProtocol, CipherSuite, DataEncoding, ProtocolVersion, SignatureScheme,
        client::{ClientHello, ClientHelloExtension},
    },
};
use rustls::pki_types;
use std::net::IpAddr;

macro_rules! enum_from_rustls {
    ($t:ty => $($name:ident),+$(,)?) => {
        $(
            impl RamaFrom<rustls::$name> for rama_net::tls::$name {
                fn rama_from(value: ::rustls::$name) -> Self {
                    let n: $t = value.into();
                    n.into()
                }
            }

            impl RamaFrom<rama_net::tls::$name> for rustls::$name {
                fn rama_from(value: rama_net::tls::$name) -> Self {
                    let n: $t = value.into();
                    n.into()
                }
            }
        )+
    };
}

enum_from_rustls!(u16 => ProtocolVersion, CipherSuite, SignatureScheme);

impl RamaTryFrom<ProtocolVersion> for &rustls::SupportedProtocolVersion {
    type Error = ProtocolVersion;

    fn rama_try_from(value: ProtocolVersion) -> Result<Self, Self::Error> {
        match value {
            ProtocolVersion::TLSv1_2 => Ok(&rustls::version::TLS12),
            ProtocolVersion::TLSv1_3 => Ok(&rustls::version::TLS13),
            other => Err(other),
        }
    }
}

impl<'a> RamaTryFrom<rustls::pki_types::ServerName<'a>> for Host {
    type Error = OpaqueError;

    fn rama_try_from(value: rustls::pki_types::ServerName<'a>) -> Result<Self, Self::Error> {
        match value {
            rustls::pki_types::ServerName::DnsName(name) => {
                Ok(Domain::try_from(name.as_ref().to_owned())?.into())
            }
            rustls::pki_types::ServerName::IpAddress(ip) => Ok(Self::from(IpAddr::from(ip))),
            _ => Err(OpaqueError::from_display(format!(
                "urecognised rustls (PKI) server name: {value:?}",
            ))),
        }
    }
}
impl RamaTryFrom<Host> for rustls::pki_types::ServerName<'_> {
    type Error = OpaqueError;

    fn rama_try_from(value: Host) -> Result<Self, Self::Error> {
        match value {
            Host::Name(name) => Ok(rustls::pki_types::ServerName::DnsName(
                rustls::pki_types::DnsName::try_from(name.as_str().to_owned())
                    .context("convert domain to rustls (PKI) ServerName")?,
            )),
            Host::Address(ip) => Ok(rustls::pki_types::ServerName::IpAddress(ip.into())),
        }
    }
}

impl<'a> RamaTryFrom<&rustls::pki_types::ServerName<'a>> for Host {
    type Error = OpaqueError;

    fn rama_try_from(value: &rustls::pki_types::ServerName<'a>) -> Result<Self, Self::Error> {
        match value {
            rustls::pki_types::ServerName::DnsName(name) => {
                Ok(Domain::try_from(name.as_ref().to_owned())?.into())
            }
            rustls::pki_types::ServerName::IpAddress(ip) => Ok(Self::from(IpAddr::from(*ip))),
            _ => Err(OpaqueError::from_display(format!(
                "urecognised rustls (PKI) server name: {value:?}",
            ))),
        }
    }
}

impl<'a> RamaTryFrom<&'a Host> for rustls::pki_types::ServerName<'a> {
    type Error = OpaqueError;

    fn rama_try_from(value: &'a Host) -> Result<Self, Self::Error> {
        match value {
            Host::Name(name) => Ok(rustls::pki_types::ServerName::DnsName(
                rustls::pki_types::DnsName::try_from(name.as_str())
                    .context("convert domain to rustls (PKI) ServerName")?,
            )),
            Host::Address(ip) => Ok(rustls::pki_types::ServerName::IpAddress((*ip).into())),
        }
    }
}

impl RamaFrom<&pki_types::CertificateDer<'static>> for DataEncoding {
    fn rama_from(value: &pki_types::CertificateDer<'static>) -> Self {
        Self::Der(value.as_ref().into())
    }
}

impl RamaFrom<&[pki_types::CertificateDer<'static>]> for DataEncoding {
    fn rama_from(value: &[pki_types::CertificateDer<'static>]) -> Self {
        Self::DerStack(
            value
                .iter()
                .map(|cert| Into::<Vec<u8>>::into(cert.as_ref()))
                .collect(),
        )
    }
}

impl<'a> RamaFrom<rustls::server::ClientHello<'a>> for ClientHello {
    fn rama_from(value: rustls::server::ClientHello<'a>) -> Self {
        let cipher_suites = value
            .cipher_suites()
            .iter()
            .map(|cs| CipherSuite::rama_from(*cs))
            .collect();

        let mut extensions = Vec::with_capacity(3);

        extensions.push(ClientHelloExtension::SignatureAlgorithms(
            value
                .signature_schemes()
                .iter()
                .map(|sc| SignatureScheme::rama_from(*sc))
                .collect(),
        ));

        if let Some(domain) = value.server_name().and_then(|d| d.parse().ok()) {
            extensions.push(ClientHelloExtension::ServerName(Some(domain)));
        }

        if let Some(alpn) = value.alpn() {
            extensions.push(ClientHelloExtension::ApplicationLayerProtocolNegotiation(
                alpn.map(ApplicationProtocol::from).collect(),
            ));
        }

        Self::new(
            // TODO: support if rustls can handle this
            ProtocolVersion::Unknown(0),
            cipher_suites,
            vec![],
            extensions,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rustls_to_common_to_rustls() {
        let p = rustls::ProtocolVersion::TLSv1_3;
        let p = ProtocolVersion::rama_from(p);
        assert_eq!(p, ProtocolVersion::TLSv1_3);
        let p = rustls::ProtocolVersion::rama_from(p);
        assert_eq!(p, rustls::ProtocolVersion::TLSv1_3);
    }
}
