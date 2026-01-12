use crate::dep::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use crate::dep::rcgen::{self, KeyPair};
use crate::dep::rustls::RootCertStore;
use crate::dep::rustls::{ALL_VERSIONS, ClientConfig};
use crate::key_log::KeyLogFile;
use crate::verify::NoServerCertVerifier;
use rama_core::error::{BoxError, ErrorContext, OpaqueError};
use rama_net::address::Host;
use rama_net::tls::{ApplicationProtocol, KeyLogIntent};
use rustls::client::danger::ServerCertVerifier;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone)]
/// Internal data used as configuration/input for the [`super::HttpsConnector`].
///
/// Created by converting a [`rustls::ClientConfig`] into it directly,
/// or by using [`TlsConnectorDataBuilder`] to build this in a more ergonomic way.
pub struct TlsConnectorData {
    pub client_config: Arc<ClientConfig>,
    pub server_name: Option<Host>,
    pub store_server_certificate_chain: bool,
}

impl From<ClientConfig> for TlsConnectorData {
    #[inline]
    fn from(value: ClientConfig) -> Self {
        Arc::new(value).into()
    }
}

impl From<Arc<ClientConfig>> for TlsConnectorData {
    fn from(value: Arc<ClientConfig>) -> Self {
        Self {
            client_config: value,
            server_name: None,
            store_server_certificate_chain: false,
        }
    }
}

impl TlsConnectorData {
    /// Create a default [`TlsConnectorData`] that is focussed
    /// on providing auto http connections, meaning supporting
    /// the http connections which `rama` supports out of the box.
    pub fn new_http_auto() -> Result<Self, OpaqueError> {
        Ok(TlsConnectorDataBuilder::new()
            .with_env_key_logger()?
            .with_alpn_protocols_http_auto()
            .build())
    }

    /// Create a default [`TlsConnectorData`] that is focussed
    /// on providing http/1.1 connections.
    pub fn new_http_1() -> Result<Self, OpaqueError> {
        Ok(TlsConnectorDataBuilder::new()
            .with_env_key_logger()?
            .with_alpn_protocols(&[ApplicationProtocol::HTTP_11])
            .build())
    }

    /// Create a default [`TlsConnectorData`] that is focussed
    /// on providing h2 connections.
    pub fn new_http_2() -> Result<Self, OpaqueError> {
        Ok(TlsConnectorDataBuilder::new()
            .with_env_key_logger()?
            .with_alpn_protocols(&[ApplicationProtocol::HTTP_2])
            .build())
    }
}

/// [`ClientConfigBuilder`] can be used to construct [`rustls::ClientConfig`] for most common use cases in Rama.
///
/// If this doesn't work for your use case, no problem [`TlsConnectorData`] can be created from a raw [`rustls::ClientConfig`]
pub struct TlsConnectorDataBuilder {
    client_config: rustls::ClientConfig,
    server_name: Option<Host>,
    store_server_certificate_chain: bool,
}

impl Default for TlsConnectorDataBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ClientConfig> for TlsConnectorDataBuilder {
    fn from(value: ClientConfig) -> Self {
        Self {
            client_config: value,
            ..Default::default()
        }
    }
}

impl TlsConnectorDataBuilder {
    /// Create a [`TlsConnectorDataBuilder`] with a starting config of: support for all tls versions, global root
    /// certificate store, and no client auth
    #[must_use]
    pub fn new() -> Self {
        let config = ClientConfig::builder_with_protocol_versions(ALL_VERSIONS)
            .with_root_certificates(client_root_certs())
            .with_no_client_auth();
        Self {
            client_config: config,
            server_name: None,
            store_server_certificate_chain: false,
        }
    }

    /// Create a [`TlsConnectorDataBuilder`] with a starting config of: support for all tls versions, global root
    /// certificate store, and with client auth
    pub fn new_with_client_auth(
        client_cert_chain: Vec<CertificateDer<'static>>,
        client_priv_key: PrivateKeyDer<'static>,
    ) -> Result<Self, BoxError> {
        let config = ClientConfig::builder_with_protocol_versions(ALL_VERSIONS)
            .with_root_certificates(client_root_certs())
            .with_client_auth_cert(client_cert_chain, client_priv_key)
            .map_err(Into::<BoxError>::into)?;

        Ok(Self {
            client_config: config,
            server_name: None,
            store_server_certificate_chain: false,
        })
    }

    /// If [`KeyLogIntent::Environment`] is set to a path, create a key logger that will write to that path
    /// and set it in the current config
    pub fn set_env_key_logger(&mut self) -> Result<&mut Self, OpaqueError> {
        if let Some(path) = KeyLogIntent::Environment.file_path().as_deref() {
            let key_logger = Arc::new(KeyLogFile::new(path)?);
            self.client_config.key_log = key_logger;
        };
        Ok(self)
    }

    /// Same as [`Self::set_env_key_logger`] but consuming self
    pub fn with_env_key_logger(mut self) -> Result<Self, OpaqueError> {
        self.set_env_key_logger()?;
        Ok(self)
    }

    /// Set [`ApplicationProtocol`]s supported in alpn extension
    pub fn set_alpn_protocols(&mut self, protos: &[ApplicationProtocol]) -> &mut Self {
        self.client_config.alpn_protocols = protos
            .iter()
            .map(|proto| proto.as_bytes().to_vec())
            .collect();

        self
    }

    /// Same as [`Self::set_alpn_protocols`] but consuming self
    #[must_use]
    pub fn with_alpn_protocols(mut self, protos: &[ApplicationProtocol]) -> Self {
        self.set_alpn_protocols(protos);
        self
    }

    /// Set alpn protocols to most commonly used http protocols: [`ApplicationProtocol::HTTP_2`], [`ApplicationProtocol::HTTP_11`]
    pub fn set_alpn_protocols_http_auto(&mut self) -> &mut Self {
        self.set_alpn_protocols(&[ApplicationProtocol::HTTP_2, ApplicationProtocol::HTTP_11]);
        self
    }

    /// Same as [`Self::set_alpn_protocols_http_auto`] but consuming self
    #[must_use]
    pub fn with_alpn_protocols_http_auto(mut self) -> Self {
        self.set_alpn_protocols_http_auto();
        self
    }

    /// Set certificate verifier that will be used to verify certs
    pub fn set_cert_verifier(&mut self, verifier: Arc<dyn ServerCertVerifier>) -> &mut Self {
        self.client_config
            .dangerous()
            .set_certificate_verifier(verifier);
        self
    }

    /// Same as [`Self::set_cert_verifier`] but consuming self
    #[must_use]
    pub fn with_cert_verifier(mut self, verifier: Arc<dyn ServerCertVerifier>) -> Self {
        self.set_cert_verifier(verifier);
        self
    }

    /// Set certificate verifier to a custom one that will allow all certificates, resulting
    /// in certificates not being verified.
    pub fn set_no_cert_verifier(&mut self) -> &mut Self {
        self.set_cert_verifier(Arc::new(NoServerCertVerifier::default()))
    }

    /// Same as [`Self::set_no_cert_verifier`] but consuming self
    #[must_use]
    pub fn with_no_cert_verifier(mut self) -> Self {
        self.set_no_cert_verifier();
        self
    }

    /// Set servername that will be used for SNI
    pub fn set_server_name(&mut self, server_name: Host) -> &mut Self {
        self.server_name = Some(server_name);
        self
    }

    /// Same as [`Self::set_server_name`] but consuming self
    #[must_use]
    pub fn with_server_name(mut self, server_name: Host) -> Self {
        self.set_server_name(server_name);
        self
    }

    /// Set server_name on this config to the provided option consuming self
    #[must_use]
    pub fn maybe_with_server_name(mut self, server_name: Option<Host>) -> Self {
        self.server_name = server_name;
        self
    }

    /// Set if server certificate should be stored in ctx
    pub fn set_store_server_certificate_chain(&mut self, value: bool) -> &mut Self {
        self.store_server_certificate_chain = value;
        self
    }

    /// Same as [`Self::set_store_server_certificate_chain`] but consuming self
    #[must_use]
    pub fn with_store_server_certificate_chain(mut self, value: bool) -> Self {
        self.set_store_server_certificate_chain(value);
        self
    }

    /// Build [`TlsConnectorData`] from the current config
    #[must_use]
    pub fn build(self) -> TlsConnectorData {
        TlsConnectorData {
            client_config: Arc::new(self.client_config),
            server_name: self.server_name,
            store_server_certificate_chain: self.store_server_certificate_chain,
        }
    }
}

pub fn client_root_certs() -> Arc<RootCertStore> {
    static ROOT_CERTS: OnceLock<Arc<RootCertStore>> = OnceLock::new();
    ROOT_CERTS
        .get_or_init(|| {
            let mut root_storage = RootCertStore::empty();
            root_storage.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            Arc::new(root_storage)
        })
        .clone()
}

pub fn self_signed_client_auth()
-> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), OpaqueError> {
    // Create a client end entity cert.
    let alg = &rcgen::PKCS_ECDSA_P256_SHA256;
    let client_key_pair =
        KeyPair::generate_for(alg).context("self-signed client auth: generate client key pair")?;
    let mut client_ee_params = rcgen::CertificateParams::new(vec![])
        .context("self-signed client auth: create client EE Params")?;
    client_ee_params.is_ca = rcgen::IsCa::NoCa;
    client_ee_params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ClientAuth];

    let client_cert = client_ee_params
        .self_signed(&client_key_pair)
        .context("create client self-signed cert")?;
    let client_cert_der = client_cert.into();
    let client_key_der = PrivatePkcs8KeyDer::from(client_key_pair.serialize_der());

    Ok((
        vec![client_cert_der],
        PrivatePkcs8KeyDer::from(client_key_der.secret_pkcs8_der().to_owned()).into(),
    ))
}
