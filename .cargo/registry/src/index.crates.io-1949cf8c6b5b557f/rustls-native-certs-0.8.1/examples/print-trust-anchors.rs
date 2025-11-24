//! Print the Subject of all extracted trust anchors.

use std::error::Error;
use x509_parser::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    for cert in rustls_native_certs::load_native_certs().expect("could not load platform certs") {
        match parse_x509_certificate(cert.as_ref()) {
            Ok((_, cert)) => println!("{}", cert.tbs_certificate.subject),
            Err(e) => eprintln!("error parsing certificate: {}", e),
        };
    }
    Ok(())
}
