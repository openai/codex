extern crate libc;
extern crate security_framework;
extern crate security_framework_sys;

use self::security_framework::base;
use self::security_framework::certificate::SecCertificate;
use self::security_framework::identity::SecIdentity;
use self::security_framework::import_export::{ImportedIdentity, Pkcs12ImportOptions};
use self::security_framework::random::SecRandom;
use self::security_framework::secure_transport::{
    self, ClientBuilder, SslConnectionType, SslContext, SslProtocol, SslProtocolSide,
};
use self::security_framework_sys::base::{errSecIO, errSecParam};
use std::error;
use std::fmt;
use std::io;
use std::str;
use std::sync::Mutex;
use std::sync::Once;

#[cfg(not(any(
    target_os = "ios",
    target_os = "watchos",
    target_os = "tvos",
    target_os = "visionos"
)))]
use self::security_framework::os::macos::certificate::{PropertyType, SecCertificateExt};
#[cfg(not(any(
    target_os = "ios",
    target_os = "watchos",
    target_os = "tvos",
    target_os = "visionos"
)))]
use self::security_framework::os::macos::certificate_oids::CertificateOid;
#[cfg(not(any(
    target_os = "ios",
    target_os = "watchos",
    target_os = "tvos",
    target_os = "visionos"
)))]
use self::security_framework::os::macos::identity::SecIdentityExt;
#[cfg(not(any(
    target_os = "ios",
    target_os = "watchos",
    target_os = "tvos",
    target_os = "visionos"
)))]
use self::security_framework::os::macos::import_export::{
    ImportOptions, Pkcs12ImportOptionsExt, SecItems,
};
#[cfg(not(any(
    target_os = "ios",
    target_os = "watchos",
    target_os = "tvos",
    target_os = "visionos"
)))]
use self::security_framework::os::macos::keychain::{self, KeychainSettings, SecKeychain};

use {Protocol, TlsAcceptorBuilder, TlsConnectorBuilder};

static SET_AT_EXIT: Once = Once::new();

#[cfg(not(any(
    target_os = "ios",
    target_os = "watchos",
    target_os = "tvos",
    target_os = "visionos"
)))]
static TEMP_KEYCHAIN: Mutex<Option<(SecKeychain, tempfile::TempDir)>> = Mutex::new(None);

fn convert_protocol(protocol: Protocol) -> SslProtocol {
    match protocol {
        Protocol::Sslv3 => SslProtocol::SSL3,
        Protocol::Tlsv10 => SslProtocol::TLS1,
        Protocol::Tlsv11 => SslProtocol::TLS11,
        Protocol::Tlsv12 => SslProtocol::TLS12,
    }
}

pub struct Error(base::Error);

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        error::Error::source(&self.0)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, fmt)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, fmt)
    }
}

impl From<base::Error> for Error {
    fn from(error: base::Error) -> Error {
        Error(error)
    }
}

#[derive(Clone, Debug)]
pub struct Identity {
    identity: SecIdentity,
    chain: Vec<SecCertificate>,
}

impl Identity {
    #[cfg(any(
        target_os = "ios",
        target_os = "watchos",
        target_os = "tvos",
        target_os = "visionos"
    ))]
    pub fn from_pkcs8(_: &[u8], _: &[u8]) -> Result<Identity, Error> {
        panic!("Not implemented on iOS");
    }

    #[cfg(not(any(
        target_os = "ios",
        target_os = "watchos",
        target_os = "tvos",
        target_os = "visionos"
    )))]
    pub fn from_pkcs8(pem: &[u8], key: &[u8]) -> Result<Identity, Error> {
        if !key.starts_with(b"-----BEGIN PRIVATE KEY-----") {
            return Err(Error(base::Error::from(errSecParam)));
        }

        let dir = tempfile::TempDir::new().map_err(|_| Error(base::Error::from(errSecIO)))?;
        let keychain = keychain::CreateOptions::new()
            .password(&random_password()?)
            .create(dir.path().join("identity.keychain"))?;

        let mut items = SecItems::default();

        ImportOptions::new()
            .filename("key.pem")
            .items(&mut items)
            .keychain(&keychain)
            .import(key)?;

        ImportOptions::new()
            .filename("chain.pem")
            .items(&mut items)
            .keychain(&keychain)
            .import(pem)?;

        let cert = items
            .certificates
            .get(0)
            .ok_or_else(|| Error(base::Error::from(errSecParam)))?;
        let ident = SecIdentity::with_certificate(&[keychain], cert)?;
        Ok(Identity {
            identity: ident,
            chain: items.certificates.into_iter().skip(1).collect(),
        })
    }

    pub fn from_pkcs12(buf: &[u8], pass: &str) -> Result<Identity, Error> {
        let mut imports = Identity::import_options(buf, pass)?;
        let import = imports.pop().unwrap();

        let identity = import
            .identity
            .expect("Pkcs12 files must include an identity");

        // FIXME: Compare the certificates for equality using CFEqual
        let identity_cert = identity.certificate()?.to_der();

        Ok(Identity {
            identity,
            chain: import
                .cert_chain
                .unwrap_or(vec![])
                .into_iter()
                .filter(|c| c.to_der() != identity_cert)
                .collect(),
        })
    }

    #[cfg(not(any(
        target_os = "ios",
        target_os = "watchos",
        target_os = "tvos",
        target_os = "visionos"
    )))]
    fn import_options(buf: &[u8], pass: &str) -> Result<Vec<ImportedIdentity>, Error> {
        SET_AT_EXIT.call_once(|| {
            extern "C" fn atexit() {
                *TEMP_KEYCHAIN.lock().unwrap() = None;
            }
            unsafe {
                libc::atexit(atexit);
            }
        });

        let keychain = match *TEMP_KEYCHAIN.lock().unwrap() {
            Some((ref keychain, _)) => keychain.clone(),
            ref mut lock @ None => {
                let dir =
                    tempfile::TempDir::new().map_err(|_| Error(base::Error::from(errSecIO)))?;

                let mut keychain = keychain::CreateOptions::new()
                    .password(pass)
                    .create(dir.path().join("tmp.keychain"))?;
                keychain.set_settings(&KeychainSettings::new())?;

                *lock = Some((keychain.clone(), dir));
                keychain
            }
        };
        let mut import_opts = Pkcs12ImportOptions::new();
        // Method shadowed by deprecated method.
        <Pkcs12ImportOptions as Pkcs12ImportOptionsExt>::keychain(&mut import_opts, keychain);
        let imports = import_opts.passphrase(pass).import(buf)?;
        Ok(imports)
    }

    #[cfg(any(
        target_os = "ios",
        target_os = "watchos",
        target_os = "tvos",
        target_os = "visionos"
    ))]
    fn import_options(buf: &[u8], pass: &str) -> Result<Vec<ImportedIdentity>, Error> {
        let imports = Pkcs12ImportOptions::new().passphrase(pass).import(buf)?;
        Ok(imports)
    }
}

fn random_password() -> Result<String, Error> {
    use std::fmt::Write;
    let mut bytes = [0_u8; 10];
    SecRandom::default()
        .copy_bytes(&mut bytes)
        .map_err(|_| Error(base::Error::from(errSecIO)))?;
    let mut s = String::with_capacity(2 * bytes.len());
    for byte in bytes {
        write!(s, "{:02X}", byte).map_err(|_| Error(base::Error::from(errSecIO)))?;
    }
    Ok(s)
}

#[derive(Clone)]
pub struct Certificate(SecCertificate);

impl Certificate {
    pub fn from_der(buf: &[u8]) -> Result<Certificate, Error> {
        let cert = SecCertificate::from_der(buf)?;
        Ok(Certificate(cert))
    }

    #[cfg(not(any(
        target_os = "ios",
        target_os = "watchos",
        target_os = "tvos",
        target_os = "visionos"
    )))]
    pub fn from_pem(buf: &[u8]) -> Result<Certificate, Error> {
        let mut items = SecItems::default();
        ImportOptions::new().items(&mut items).import(buf)?;
        if items.certificates.len() == 1 && items.identities.is_empty() && items.keys.is_empty() {
            Ok(Certificate(items.certificates.pop().unwrap()))
        } else {
            Err(Error(base::Error::from(errSecParam)))
        }
    }

    #[cfg(any(
        target_os = "ios",
        target_os = "watchos",
        target_os = "tvos",
        target_os = "visionos"
    ))]
    pub fn from_pem(_: &[u8]) -> Result<Certificate, Error> {
        panic!("Not implemented on iOS, tvOS, watchOS or visionOS");
    }

    pub fn to_der(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_der())
    }
}

pub enum HandshakeError<S> {
    WouldBlock(MidHandshakeTlsStream<S>),
    Failure(Error),
}

impl<S> From<secure_transport::ClientHandshakeError<S>> for HandshakeError<S> {
    fn from(e: secure_transport::ClientHandshakeError<S>) -> HandshakeError<S> {
        match e {
            secure_transport::ClientHandshakeError::Failure(e) => HandshakeError::Failure(e.into()),
            secure_transport::ClientHandshakeError::Interrupted(s) => {
                HandshakeError::WouldBlock(MidHandshakeTlsStream::Client(s))
            }
        }
    }
}

impl<S> From<base::Error> for HandshakeError<S> {
    fn from(e: base::Error) -> HandshakeError<S> {
        HandshakeError::Failure(e.into())
    }
}

pub enum MidHandshakeTlsStream<S> {
    Server(
        secure_transport::MidHandshakeSslStream<S>,
        Option<SecCertificate>,
    ),
    Client(secure_transport::MidHandshakeClientBuilder<S>),
}

impl<S> fmt::Debug for MidHandshakeTlsStream<S>
where
    S: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MidHandshakeTlsStream::Server(ref s, _) => s.fmt(fmt),
            MidHandshakeTlsStream::Client(ref s) => s.fmt(fmt),
        }
    }
}

impl<S> MidHandshakeTlsStream<S> {
    pub fn get_ref(&self) -> &S {
        match *self {
            MidHandshakeTlsStream::Server(ref s, _) => s.get_ref(),
            MidHandshakeTlsStream::Client(ref s) => s.get_ref(),
        }
    }

    pub fn get_mut(&mut self) -> &mut S {
        match *self {
            MidHandshakeTlsStream::Server(ref mut s, _) => s.get_mut(),
            MidHandshakeTlsStream::Client(ref mut s) => s.get_mut(),
        }
    }
}

impl<S> MidHandshakeTlsStream<S>
where
    S: io::Read + io::Write,
{
    pub fn handshake(self) -> Result<TlsStream<S>, HandshakeError<S>> {
        match self {
            MidHandshakeTlsStream::Server(s, cert) => match s.handshake() {
                Ok(stream) => Ok(TlsStream { stream, cert }),
                Err(secure_transport::HandshakeError::Failure(e)) => {
                    Err(HandshakeError::Failure(Error(e)))
                }
                Err(secure_transport::HandshakeError::Interrupted(s)) => Err(
                    HandshakeError::WouldBlock(MidHandshakeTlsStream::Server(s, cert)),
                ),
            },
            MidHandshakeTlsStream::Client(s) => match s.handshake() {
                Ok(stream) => Ok(TlsStream { stream, cert: None }),
                Err(e) => Err(e.into()),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct TlsConnector {
    identity: Option<Identity>,
    min_protocol: Option<Protocol>,
    max_protocol: Option<Protocol>,
    roots: Vec<SecCertificate>,
    use_sni: bool,
    danger_accept_invalid_hostnames: bool,
    danger_accept_invalid_certs: bool,
    disable_built_in_roots: bool,
    #[cfg(feature = "alpn")]
    alpn: Vec<String>,
}

impl TlsConnector {
    pub fn new(builder: &TlsConnectorBuilder) -> Result<TlsConnector, Error> {
        Ok(TlsConnector {
            identity: builder.identity.as_ref().map(|i| i.0.clone()),
            min_protocol: builder.min_protocol,
            max_protocol: builder.max_protocol,
            roots: builder
                .root_certificates
                .iter()
                .map(|c| (c.0).0.clone())
                .collect(),
            use_sni: builder.use_sni,
            danger_accept_invalid_hostnames: builder.accept_invalid_hostnames,
            danger_accept_invalid_certs: builder.accept_invalid_certs,
            disable_built_in_roots: builder.disable_built_in_roots,
            #[cfg(feature = "alpn")]
            alpn: builder.alpn.clone(),
        })
    }

    pub fn connect<S>(&self, domain: &str, stream: S) -> Result<TlsStream<S>, HandshakeError<S>>
    where
        S: io::Read + io::Write,
    {
        let mut builder = ClientBuilder::new();
        if let Some(min) = self.min_protocol {
            builder.protocol_min(convert_protocol(min));
        }
        if let Some(max) = self.max_protocol {
            builder.protocol_max(convert_protocol(max));
        }
        if let Some(identity) = self.identity.as_ref() {
            builder.identity(&identity.identity, &identity.chain);
        }
        builder.anchor_certificates(&self.roots);
        builder.use_sni(self.use_sni);
        builder.danger_accept_invalid_hostnames(self.danger_accept_invalid_hostnames);
        builder.danger_accept_invalid_certs(self.danger_accept_invalid_certs);
        builder.trust_anchor_certificates_only(self.disable_built_in_roots);

        #[cfg(feature = "alpn")]
        {
            if !self.alpn.is_empty() {
                builder.alpn_protocols(&self.alpn.iter().map(String::as_str).collect::<Vec<_>>());
            }
        }

        match builder.handshake(domain, stream) {
            Ok(stream) => Ok(TlsStream { stream, cert: None }),
            Err(e) => Err(e.into()),
        }
    }
}

#[derive(Clone)]
pub struct TlsAcceptor {
    identity: Identity,
    min_protocol: Option<Protocol>,
    max_protocol: Option<Protocol>,
}

impl TlsAcceptor {
    pub fn new(builder: &TlsAcceptorBuilder) -> Result<TlsAcceptor, Error> {
        Ok(TlsAcceptor {
            identity: builder.identity.0.clone(),
            min_protocol: builder.min_protocol,
            max_protocol: builder.max_protocol,
        })
    }

    pub fn accept<S>(&self, stream: S) -> Result<TlsStream<S>, HandshakeError<S>>
    where
        S: io::Read + io::Write,
    {
        let mut ctx = SslContext::new(SslProtocolSide::SERVER, SslConnectionType::STREAM)?;

        if let Some(min) = self.min_protocol {
            ctx.set_protocol_version_min(convert_protocol(min))?;
        }
        if let Some(max) = self.max_protocol {
            ctx.set_protocol_version_max(convert_protocol(max))?;
        }
        ctx.set_certificate(&self.identity.identity, &self.identity.chain)?;
        let cert = Some(self.identity.identity.certificate()?);
        match ctx.handshake(stream) {
            Ok(stream) => Ok(TlsStream { stream, cert }),
            Err(secure_transport::HandshakeError::Failure(e)) => {
                Err(HandshakeError::Failure(Error(e)))
            }
            Err(secure_transport::HandshakeError::Interrupted(s)) => Err(
                HandshakeError::WouldBlock(MidHandshakeTlsStream::Server(s, cert)),
            ),
        }
    }
}

pub struct TlsStream<S> {
    stream: secure_transport::SslStream<S>,
    cert: Option<SecCertificate>,
}

impl<S: fmt::Debug> fmt::Debug for TlsStream<S> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.stream, fmt)
    }
}

impl<S> TlsStream<S> {
    pub fn get_ref(&self) -> &S {
        self.stream.get_ref()
    }

    pub fn get_mut(&mut self) -> &mut S {
        self.stream.get_mut()
    }
}

impl<S: io::Read + io::Write> TlsStream<S> {
    pub fn buffered_read_size(&self) -> Result<usize, Error> {
        Ok(self.stream.context().buffered_read_size()?)
    }

    #[allow(deprecated)]
    pub fn peer_certificate(&self) -> Result<Option<Certificate>, Error> {
        let trust = match self.stream.context().peer_trust2()? {
            Some(trust) => trust,
            None => return Ok(None),
        };
        trust.evaluate()?;

        Ok(trust.certificate_at_index(0).map(Certificate))
    }

    #[cfg(feature = "alpn")]
    pub fn negotiated_alpn(&self) -> Result<Option<Vec<u8>>, Error> {
        match self.stream.context().alpn_protocols() {
            Ok(protocols) => {
                // Per RFC7301, "ProtocolNameList" MUST contain exactly one "ProtocolName".
                assert!(protocols.len() < 2);

                if protocols.is_empty() {
                    // Not sure this is actually possible.
                    Ok(None)
                } else {
                    Ok(Some(protocols.into_iter().next().unwrap().into_bytes()))
                }
            }
            // The macOS API appears to return `errSecParam` whenever no ALPN was negotiated, both
            // when it isn't attempted and when it isn't successful.
            Err(e) if e.code() == errSecParam => Ok(None),
            Err(other) => Err(Error::from(other)),
        }
    }

    #[cfg(any(
        target_os = "ios",
        target_os = "watchos",
        target_os = "tvos",
        target_os = "visionos"
    ))]
    pub fn tls_server_end_point(&self) -> Result<Option<Vec<u8>>, Error> {
        Ok(None)
    }

    #[cfg(not(any(
        target_os = "ios",
        target_os = "watchos",
        target_os = "tvos",
        target_os = "visionos"
    )))]
    pub fn tls_server_end_point(&self) -> Result<Option<Vec<u8>>, Error> {
        let cert = match self.cert {
            Some(ref cert) => cert.clone(),
            None => match self.peer_certificate()? {
                Some(cert) => cert.0,
                None => return Ok(None),
            },
        };

        let property = match cert
            .properties(Some(&[CertificateOid::x509_v1_signature_algorithm()]))
            .ok()
            .and_then(|p| p.get(CertificateOid::x509_v1_signature_algorithm()))
        {
            Some(property) => property,
            None => return Ok(None),
        };

        let section = match property.get() {
            PropertyType::Section(section) => section,
            _ => return Ok(None),
        };

        let algorithm = match section.iter().find(|p| p.label() == "Algorithm") {
            Some(property) => property,
            None => return Ok(None),
        };

        let algorithm = match algorithm.get() {
            PropertyType::String(algorithm) => algorithm,
            _ => return Ok(None),
        };

        let digest = match &*algorithm.to_string() {
            // MD5
            "1.2.840.113549.2.5" | "1.2.840.113549.1.1.4" | "1.3.14.3.2.3" => Digest::Sha256,
            // SHA-1
            "1.3.14.3.2.26"
            | "1.3.14.3.2.15"
            | "1.2.840.113549.1.1.5"
            | "1.3.14.3.2.29"
            | "1.2.840.10040.4.3"
            | "1.3.14.3.2.13"
            | "1.2.840.10045.4.1" => Digest::Sha256,
            // SHA-224
            "2.16.840.1.101.3.4.2.4"
            | "1.2.840.113549.1.1.14"
            | "2.16.840.1.101.3.4.3.1"
            | "1.2.840.10045.4.3.1" => Digest::Sha224,
            // SHA-256
            "2.16.840.1.101.3.4.2.1" | "1.2.840.113549.1.1.11" | "1.2.840.10045.4.3.2" => {
                Digest::Sha256
            }
            // SHA-384
            "2.16.840.1.101.3.4.2.2" | "1.2.840.113549.1.1.12" | "1.2.840.10045.4.3.3" => {
                Digest::Sha384
            }
            // SHA-512
            "2.16.840.1.101.3.4.2.3" | "1.2.840.113549.1.1.13" | "1.2.840.10045.4.3.4" => {
                Digest::Sha512
            }
            _ => return Ok(None),
        };

        let der = cert.to_der();
        Ok(Some(digest.hash(&der)))
    }

    pub fn shutdown(&mut self) -> io::Result<()> {
        self.stream.close()?;
        Ok(())
    }
}

impl<S: io::Read + io::Write> io::Read for TlsStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }
}

impl<S: io::Read + io::Write> io::Write for TlsStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

enum Digest {
    Sha224,
    Sha256,
    Sha384,
    Sha512,
}

impl Digest {
    fn hash(&self, data: &[u8]) -> Vec<u8> {
        unsafe {
            assert!(data.len() <= CC_LONG::max_value() as usize);
            match *self {
                Digest::Sha224 => {
                    let mut buf = [0; CC_SHA224_DIGEST_LENGTH];
                    CC_SHA224(data.as_ptr(), data.len() as CC_LONG, buf.as_mut_ptr());
                    buf.to_vec()
                }
                Digest::Sha256 => {
                    let mut buf = [0; CC_SHA256_DIGEST_LENGTH];
                    CC_SHA256(data.as_ptr(), data.len() as CC_LONG, buf.as_mut_ptr());
                    buf.to_vec()
                }
                Digest::Sha384 => {
                    let mut buf = [0; CC_SHA384_DIGEST_LENGTH];
                    CC_SHA384(data.as_ptr(), data.len() as CC_LONG, buf.as_mut_ptr());
                    buf.to_vec()
                }
                Digest::Sha512 => {
                    let mut buf = [0; CC_SHA512_DIGEST_LENGTH];
                    CC_SHA512(data.as_ptr(), data.len() as CC_LONG, buf.as_mut_ptr());
                    buf.to_vec()
                }
            }
        }
    }
}

// FIXME ideally we'd pull these in from elsewhere
const CC_SHA224_DIGEST_LENGTH: usize = 28;
const CC_SHA256_DIGEST_LENGTH: usize = 32;
const CC_SHA384_DIGEST_LENGTH: usize = 48;
const CC_SHA512_DIGEST_LENGTH: usize = 64;
#[allow(non_camel_case_types)]
type CC_LONG = u32;

extern "C" {
    fn CC_SHA224(data: *const u8, len: CC_LONG, md: *mut u8) -> *mut u8;
    fn CC_SHA256(data: *const u8, len: CC_LONG, md: *mut u8) -> *mut u8;
    fn CC_SHA384(data: *const u8, len: CC_LONG, md: *mut u8) -> *mut u8;
    fn CC_SHA512(data: *const u8, len: CC_LONG, md: *mut u8) -> *mut u8;
}
