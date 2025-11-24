extern crate native_tls;

use native_tls::{Identity, TlsAcceptor, TlsStream};
use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

fn main() {
    let mut cert_file = File::open("test/cert.pem").unwrap();
    let mut certs = vec![];
    cert_file.read_to_end(&mut certs).unwrap();
    let mut key_file = File::open("test/key.pem").unwrap();
    let mut key = vec![];
    key_file.read_to_end(&mut key).unwrap();
    let pkcs8 = Identity::from_pkcs8(&certs, &key).unwrap();

    let acceptor = TlsAcceptor::new(pkcs8).unwrap();
    let acceptor = Arc::new(acceptor);

    let listener = TcpListener::bind("0.0.0.0:8443").unwrap();

    fn handle_client(mut stream: TlsStream<TcpStream>) {
        let mut buf = [0; 1024];
        let read = stream.read(&mut buf).unwrap();
        let received = std::str::from_utf8(&buf[0..read]).unwrap();
        stream
            .write_all(format!("received '{}'", received).as_bytes())
            .unwrap();
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let acceptor = acceptor.clone();
                thread::spawn(move || {
                    let stream = acceptor.accept(stream).unwrap();
                    handle_client(stream);
                });
            }
            Err(_e) => { /* connection failed */ }
        }
    }
}
