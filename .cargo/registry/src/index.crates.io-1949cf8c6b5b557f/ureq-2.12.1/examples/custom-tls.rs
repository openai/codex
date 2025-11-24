use std::fmt;
use std::io;
use std::net::TcpStream;
use std::sync::Arc;

use ureq::{Error, ReadWrite, TlsConnector};

pub fn main() -> Result<(), Error> {
    let pass = PassThrough {
        handshake_fail: false,
    };

    let agent = ureq::builder().tls_connector(Arc::new(pass)).build();

    let _response = agent.get("https://httpbin.org/").call();

    // Uncomment this if handshake_fail is set to true above.
    // assert_eq!(
    //     _response.unwrap_err().to_string(),
    //     "https://httpbin.org/: Network Error: Tls handshake failed"
    // );

    Ok(())
}

/// A pass-through tls connector that just uses the plain socket without any encryption.
/// This is not a good idea for production code. The `handshake_fail` can be set to true
/// to simulate a TLS handshake failure.
struct PassThrough {
    handshake_fail: bool,
}

impl TlsConnector for PassThrough {
    fn connect(
        &self,
        _dns_name: &str,
        io: Box<dyn ReadWrite>,
    ) -> Result<Box<dyn ReadWrite>, Error> {
        if self.handshake_fail {
            let io_err = io::Error::new(io::ErrorKind::InvalidData, PassThroughError);
            return Err(io_err.into());
        }

        Ok(Box::new(CustomTlsStream(io)))
    }
}

#[derive(Debug)]
struct CustomTlsStream(Box<dyn ReadWrite>);

impl ReadWrite for CustomTlsStream {
    fn socket(&self) -> Option<&TcpStream> {
        self.0.socket()
    }
}

impl io::Read for CustomTlsStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl io::Write for CustomTlsStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[derive(Debug)]
struct PassThroughError;

impl fmt::Display for PassThroughError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tls handshake failed")
    }
}

impl std::error::Error for PassThroughError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
