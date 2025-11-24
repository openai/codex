use std::{fmt, io::Cursor};

use tokio_rustls::rustls::pki_types::{pem::PemObject as _, CertificateDer, PrivateKeyDer};

use crate::transport::{Certificate, Identity};

/// h2 alpn in plain format for rustls.
pub(crate) const ALPN_H2: &[u8] = b"h2";

#[derive(Debug)]
pub(crate) enum TlsError {
    #[cfg(feature = "channel")]
    H2NotNegotiated,
    #[cfg(feature = "tls-native-roots")]
    NativeCertsNotFound,
    CertificateParseError,
    PrivateKeyParseError,
}

impl fmt::Display for TlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "channel")]
            TlsError::H2NotNegotiated => write!(f, "HTTP/2 was not negotiated."),
            #[cfg(feature = "tls-native-roots")]
            TlsError::NativeCertsNotFound => write!(f, "no native certs found"),
            TlsError::CertificateParseError => write!(f, "Error parsing TLS certificate."),
            TlsError::PrivateKeyParseError => write!(
                f,
                "Error parsing TLS private key - no RSA or PKCS8-encoded keys found."
            ),
        }
    }
}

impl std::error::Error for TlsError {}

pub(crate) fn convert_certificate_to_pki_types(
    certificate: &Certificate,
) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    CertificateDer::pem_reader_iter(&mut Cursor::new(certificate))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| TlsError::CertificateParseError)
}

pub(crate) fn convert_identity_to_pki_types(
    identity: &Identity,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), TlsError> {
    let cert = convert_certificate_to_pki_types(&identity.cert)?;
    let key = PrivateKeyDer::from_pem_reader(&mut Cursor::new(&identity.key))
        .map_err(|_| TlsError::PrivateKeyParseError)?;
    Ok((cert, key))
}
