use crate::error::Error;
use crate::error::ErrorKind;
use crate::stream::{ReadWrite, TlsConnector};

use std::net::TcpStream;
use std::sync::Arc;

#[allow(dead_code)]
pub(crate) fn default_tls_config() -> std::sync::Arc<dyn TlsConnector> {
    Arc::new(native_tls::TlsConnector::new().unwrap())
}

impl TlsConnector for native_tls::TlsConnector {
    fn connect(&self, dns_name: &str, io: Box<dyn ReadWrite>) -> Result<Box<dyn ReadWrite>, Error> {
        let stream =
            native_tls::TlsConnector::connect(self, dns_name, io).map_err(|e| match e {
                native_tls::HandshakeError::Failure(e) => ErrorKind::ConnectionFailed
                    .msg("native_tls connect failed")
                    .src(e),
                native_tls::HandshakeError::WouldBlock(_) => ErrorKind::Io
                    .msg("native_tls handshake timed out")
                    .src(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "native_tls handshake timed out",
                    )),
            })?;

        Ok(Box::new(stream))
    }
}

#[cfg(feature = "native-tls")]
impl ReadWrite for native_tls::TlsStream<Box<dyn ReadWrite>> {
    fn socket(&self) -> Option<&TcpStream> {
        self.get_ref().socket()
    }
}
