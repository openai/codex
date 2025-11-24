use crate::connection::Connection;
use crate::util::refined_tcp_stream::Stream as RefinedStream;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

pub(crate) struct OpenSslStream {
    inner: openssl::ssl::SslStream<Connection>,
}

/// An OpenSSL stream which has been split into two mutually exclusive streams (e.g. for read / write)
pub(crate) struct SplitOpenSslStream(Arc<Mutex<OpenSslStream>>);

// These struct methods form the implict contract for swappable TLS implementations
impl SplitOpenSslStream {
    pub(crate) fn peer_addr(&mut self) -> std::io::Result<Option<SocketAddr>> {
        self.0.lock().unwrap().inner.get_mut().peer_addr()
    }

    pub(crate) fn shutdown(&mut self, how: Shutdown) -> std::io::Result<()> {
        self.0.lock().unwrap().inner.get_mut().shutdown(how)
    }
}

impl Clone for SplitOpenSslStream {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Read for SplitOpenSslStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

impl Write for SplitOpenSslStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

impl Read for OpenSslStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for OpenSslStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

pub(crate) struct OpenSslContext(openssl::ssl::SslContext);

impl OpenSslContext {
    pub fn from_pem(
        certificates: Vec<u8>,
        private_key: Zeroizing<Vec<u8>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        use openssl::pkey::PKey;
        use openssl::ssl::{self, SslVerifyMode};
        use openssl::x509::X509;

        let mut ctx = openssl::ssl::SslContext::builder(ssl::SslMethod::tls())?;
        ctx.set_cipher_list("DEFAULT")?;
        let certificate_chain = X509::stack_from_pem(&certificates)?;
        if certificate_chain.is_empty() {
            return Err("Couldn't extract certificate chain from config.".into());
        }
        // The leaf certificate must always be first in the PEM file
        ctx.set_certificate(&certificate_chain[0])?;
        for chain_cert in certificate_chain.into_iter().skip(1) {
            ctx.add_extra_chain_cert(chain_cert)?;
        }
        let key = PKey::private_key_from_pem(&private_key)?;
        ctx.set_private_key(&key)?;
        ctx.set_verify(SslVerifyMode::NONE);
        ctx.check_private_key()?;

        Ok(Self(ctx.build()))
    }

    pub fn accept(
        &self,
        stream: Connection,
    ) -> Result<OpenSslStream, Box<dyn Error + Send + Sync + 'static>> {
        use openssl::ssl::Ssl;
        let session = Ssl::new(&self.0).expect("Failed to create new OpenSSL session");
        let stream = session.accept(stream)?;
        Ok(OpenSslStream { inner: stream })
    }
}

impl From<OpenSslStream> for RefinedStream {
    fn from(stream: OpenSslStream) -> Self {
        RefinedStream::Https(SplitOpenSslStream(Arc::new(Mutex::new(stream))))
    }
}
