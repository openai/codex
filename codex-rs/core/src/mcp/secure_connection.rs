// Phase 1: TLS 1.3 + mTLS for MCP Server Communication
// Based on design document: _docs/2025-10-28_セキュア通信アーキテクチャ設計書.md

use anyhow::{Context, Result};
use rustls::{ClientConfig, RootCertStore};
use rustls_pemfile::{certs, rsa_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// TLS configuration for MCP connections
#[derive(Debug, Clone)]
pub struct SecureMcpConfig {
    /// Path to CA certificate (root of trust)
    pub ca_cert_path: String,

    /// Path to client certificate (for mTLS)
    pub client_cert_path: Option<String>,

    /// Path to client private key (for mTLS)
    pub client_key_path: Option<String>,

    /// Verify peer certificate (should always be true in production)
    pub verify_peer: bool,

    /// TLS version (only "1.3" supported)
    pub tls_version: String,
}

impl Default for SecureMcpConfig {
    fn default() -> Self {
        Self {
            ca_cert_path: String::from("~/.codex/certs/ca/ca-cert.pem"),
            client_cert_path: None,
            client_key_path: None,
            verify_peer: true,
            tls_version: String::from("1.3"),
        }
    }
}

/// Secure MCP Connection Manager (TLS 1.3 + mTLS)
pub struct SecureMcpConnectionManager {
    /// TLS configuration
    tls_config: Arc<ClientConfig>,
}

impl SecureMcpConnectionManager {
    /// Create a new secure connection manager
    pub fn new(config: &SecureMcpConfig) -> Result<Self> {
        let tls_config = Self::build_tls_config(config)?;

        Ok(Self {
            tls_config: Arc::new(tls_config),
        })
    }

    /// Build TLS 1.3 configuration
    fn build_tls_config(config: &SecureMcpConfig) -> Result<ClientConfig> {
        // Load CA certificates (root of trust)
        let ca_certs = Self::load_ca_certificates(&config.ca_cert_path)?;

        let mut root_store = RootCertStore::empty();
        for cert in ca_certs {
            root_store
                .add(cert)
                .context("Failed to add CA cert to root store")?;
        }

        // Build TLS config based on whether mTLS is enabled
        let tls_config = if let (Some(client_cert_path), Some(client_key_path)) =
            (&config.client_cert_path, &config.client_key_path)
        {
            // mTLS: Load client certificate and key
            let client_certs = Self::load_certificates(client_cert_path)?;
            let client_key = Self::load_private_key(client_key_path)?;

            ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_client_auth_cert(client_certs, client_key)
                .context("Failed to configure TLS with client auth")?
        } else {
            // TLS only (no client certificate)
            ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth()
        };

        Ok(tls_config)
    }

    /// Load CA certificates from PEM file
    fn load_ca_certificates(path: &str) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
        let path = Self::expand_path(path);
        let file = File::open(&path)
            .with_context(|| format!("Failed to open CA certificate file: {}", path))?;
        let mut reader = BufReader::new(file);

        let certs: Vec<_> = certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to parse CA certificates")?;

        if certs.is_empty() {
            anyhow::bail!("No certificates found in CA file: {}", path);
        }

        Ok(certs)
    }

    /// Load client certificates from PEM file
    fn load_certificates(path: &str) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
        let path = Self::expand_path(path);
        let file = File::open(&path)
            .with_context(|| format!("Failed to open certificate file: {}", path))?;
        let mut reader = BufReader::new(file);

        let certs: Vec<_> = certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to parse certificates")?;

        if certs.is_empty() {
            anyhow::bail!("No certificates found in file: {}", path);
        }

        Ok(certs)
    }

    /// Load private key from PEM file
    fn load_private_key(path: &str) -> Result<rustls::pki_types::PrivateKeyDer<'static>> {
        let path = Self::expand_path(path);
        let file = File::open(&path)
            .with_context(|| format!("Failed to open private key file: {}", path))?;
        let mut reader = BufReader::new(file);

        // Try RSA keys first
        let keys = rsa_private_keys(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to parse RSA private key")?;

        if keys.is_empty() {
            anyhow::bail!("No private keys found in file: {}", path);
        }

        // Return first key
        Ok(rustls::pki_types::PrivateKeyDer::Pkcs1(
            keys.into_iter().next().unwrap(),
        ))
    }

    /// Expand ~ in path to home directory
    fn expand_path(path: &str) -> String {
        if path.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(&path[2..]).to_string_lossy().to_string();
            }
        }
        path.to_string()
    }

    /// Connect to MCP server with TLS 1.3
    pub async fn connect_tls(
        &self,
        server_name: &str,
        server_addr: &str,
    ) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
        // Establish TCP connection
        let tcp_stream = TcpStream::connect(server_addr)
            .await
            .with_context(|| format!("Failed to connect to {}", server_addr))?;

        // Create TLS connector
        let connector = TlsConnector::from(Arc::clone(&self.tls_config));

        // Perform TLS handshake
        let domain = rustls::pki_types::ServerName::try_from(server_name.to_owned())
            .context("Invalid server name")?;

        let tls_stream = connector
            .connect(domain, tcp_stream)
            .await
            .context("TLS handshake failed")?;

        Ok(tls_stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SecureMcpConfig::default();
        assert_eq!(config.tls_version, "1.3");
        assert!(config.verify_peer);
    }

    #[test]
    fn test_expand_path() {
        let expanded = SecureMcpConnectionManager::expand_path("~/test.pem");
        assert!(!expanded.starts_with("~/"));
    }
}
