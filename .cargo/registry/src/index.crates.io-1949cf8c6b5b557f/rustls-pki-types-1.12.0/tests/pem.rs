#![cfg(feature = "std")]

use std::io::Cursor;

use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{
    CertificateDer, CertificateRevocationListDer, CertificateSigningRequestDer, EchConfigListBytes,
    PrivateKeyDer, PrivatePkcs1KeyDer, PrivatePkcs8KeyDer, PrivateSec1KeyDer,
    SubjectPublicKeyInfoDer, pem,
};

#[test]
fn pkcs1_private_key() {
    let data = include_bytes!("data/zen.pem");

    PrivatePkcs1KeyDer::from_pem_slice(data).unwrap();
    PrivatePkcs1KeyDer::from_pem_reader(&mut Cursor::new(&data[..])).unwrap();
    PrivatePkcs1KeyDer::from_pem_file("tests/data/zen.pem").unwrap();

    assert!(matches!(
        PrivatePkcs1KeyDer::from_pem_file("tests/data/certificate.chain.pem").unwrap_err(),
        pem::Error::NoItemsFound
    ));
}

#[test]
fn pkcs8_private_key() {
    let data = include_bytes!("data/zen.pem");

    PrivatePkcs8KeyDer::from_pem_slice(data).unwrap();
    PrivatePkcs8KeyDer::from_pem_reader(&mut Cursor::new(&data[..])).unwrap();
    PrivatePkcs8KeyDer::from_pem_file("tests/data/zen.pem").unwrap();

    assert!(matches!(
        PrivatePkcs8KeyDer::from_pem_file("tests/data/certificate.chain.pem").unwrap_err(),
        pem::Error::NoItemsFound
    ));
}

#[test]
fn sec1_private_key() {
    let data = include_bytes!("data/zen.pem");

    PrivateSec1KeyDer::from_pem_slice(data).unwrap();
    PrivateSec1KeyDer::from_pem_reader(&mut Cursor::new(&data[..])).unwrap();
    PrivateSec1KeyDer::from_pem_file("tests/data/zen.pem").unwrap();

    assert!(matches!(
        PrivateSec1KeyDer::from_pem_file("tests/data/certificate.chain.pem").unwrap_err(),
        pem::Error::NoItemsFound
    ));
}

#[test]
fn any_private_key() {
    let data = include_bytes!("data/zen.pem");

    PrivateKeyDer::from_pem_slice(data).unwrap();
    PrivateKeyDer::from_pem_reader(&mut Cursor::new(&data[..])).unwrap();
    PrivateKeyDer::from_pem_file("tests/data/zen.pem").unwrap();

    for other_file in [
        "tests/data/nistp256key.pem",
        "tests/data/nistp256key.pkcs8.pem",
        "tests/data/rsa1024.pkcs1.pem",
        "tests/data/rsa1024.pkcs8.pem",
    ] {
        PrivateKeyDer::from_pem_file(other_file).unwrap();
    }

    assert!(matches!(
        PrivateKeyDer::from_pem_file("tests/data/certificate.chain.pem").unwrap_err(),
        pem::Error::NoItemsFound
    ));
}

#[test]
fn no_trailing_newline() {
    let data = include_bytes!("data/rsa-key-no-trailing-newline.pem");
    assert_eq!(
        PrivatePkcs8KeyDer::pem_slice_iter(data)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        1
    );

    assert_eq!(
        PrivatePkcs8KeyDer::pem_file_iter("tests/data/rsa-key-no-trailing-newline.pem")
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn certificates() {
    let data = include_bytes!("data/zen.pem");

    assert_eq!(
        CertificateDer::pem_slice_iter(data)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        CertificateDer::pem_reader_iter(&mut Cursor::new(&data[..]))
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        CertificateDer::pem_file_iter("tests/data/zen.pem")
            .unwrap()
            .count(),
        4
    );

    assert!(matches!(
        CertificateDer::from_pem_file("tests/data/crl.pem").unwrap_err(),
        pem::Error::NoItemsFound
    ));

    assert_eq!(
        CertificateDer::pem_file_iter("tests/data/certificate.chain.pem")
            .unwrap()
            .count(),
        3
    );
}

#[test]
fn public_keys() {
    let data = include_bytes!("data/spki.pem");

    SubjectPublicKeyInfoDer::from_pem_slice(data).unwrap();
    SubjectPublicKeyInfoDer::from_pem_reader(&mut Cursor::new(&data[..])).unwrap();
    SubjectPublicKeyInfoDer::from_pem_file("tests/data/spki.pem").unwrap();

    assert!(matches!(
        SubjectPublicKeyInfoDer::from_pem_file("tests/data/certificate.chain.pem").unwrap_err(),
        pem::Error::NoItemsFound
    ));
}

#[test]
fn csr() {
    let data = include_bytes!("data/zen.pem");

    CertificateSigningRequestDer::from_pem_slice(data).unwrap();
    CertificateSigningRequestDer::from_pem_reader(&mut Cursor::new(&data[..])).unwrap();
    CertificateSigningRequestDer::from_pem_file("tests/data/zen.pem").unwrap();

    assert!(matches!(
        CertificateSigningRequestDer::from_pem_file("tests/data/certificate.chain.pem")
            .unwrap_err(),
        pem::Error::NoItemsFound
    ));
}

#[test]
fn crls() {
    let data = include_bytes!("data/zen.pem");

    assert_eq!(
        CertificateRevocationListDer::pem_slice_iter(data)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        CertificateRevocationListDer::pem_reader_iter(&mut Cursor::new(&data[..]))
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        CertificateRevocationListDer::pem_file_iter("tests/data/zen.pem")
            .unwrap()
            .count(),
        1
    );

    assert!(matches!(
        CertificateRevocationListDer::pem_file_iter("tests/data/certificate.chain.pem")
            .unwrap()
            .count(),
        0
    ));

    assert_eq!(
        CertificateRevocationListDer::pem_file_iter("tests/data/crl.pem")
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn ech_config() {
    let data = include_bytes!("data/zen.pem");

    EchConfigListBytes::from_pem_slice(data).unwrap();
    EchConfigListBytes::from_pem_reader(&mut Cursor::new(&data[..])).unwrap();
    EchConfigListBytes::from_pem_file("tests/data/zen.pem").unwrap();

    assert!(matches!(
        EchConfigListBytes::from_pem_file("tests/data/certificate.chain.pem").unwrap_err(),
        pem::Error::NoItemsFound
    ));

    let (config, key) = EchConfigListBytes::config_and_key_from_iter(
        PemObject::pem_file_iter("tests/data/ech.pem").unwrap(),
    )
    .unwrap();
    println!("{config:?} {key:?}");

    assert!(matches!(
        EchConfigListBytes::config_and_key_from_iter(
            PemObject::pem_file_iter("tests/data/certificate.chain.pem").unwrap(),
        )
        .unwrap_err(),
        pem::Error::NoItemsFound,
    ));
}

#[test]
fn certificates_with_binary() {
    let data = include_bytes!("data/gunk.pem");

    assert_eq!(
        CertificateDer::pem_slice_iter(data)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        CertificateDer::pem_reader_iter(&mut Cursor::new(&data[..]))
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        CertificateDer::pem_file_iter("tests/data/gunk.pem")
            .unwrap()
            .count(),
        2
    );
}

#[test]
fn parse_in_order() {
    let data = include_bytes!("data/zen.pem");
    let items = <(pem::SectionKind, Vec<u8>) as PemObject>::pem_slice_iter(data)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(items.len(), 12);
    assert!(matches!(items[0], (pem::SectionKind::Certificate, _)));
    assert!(matches!(items[1], (pem::SectionKind::Certificate, _)));
    assert!(matches!(items[2], (pem::SectionKind::Certificate, _)));
    assert!(matches!(items[3], (pem::SectionKind::Certificate, _)));
    assert!(matches!(items[4], (pem::SectionKind::EcPrivateKey, _)));
    assert!(matches!(items[5], (pem::SectionKind::PrivateKey, _)));
    assert!(matches!(items[6], (pem::SectionKind::PublicKey, _)));
    assert!(matches!(items[7], (pem::SectionKind::RsaPrivateKey, _)));
    assert!(matches!(items[8], (pem::SectionKind::PrivateKey, _)));
    assert!(matches!(items[9], (pem::SectionKind::Crl, _)));
    assert!(matches!(items[10], (pem::SectionKind::Csr, _)));
    assert!(matches!(items[11], (pem::SectionKind::EchConfigList, _)));
}

#[test]
fn whitespace_prefix() {
    CertificateDer::from_pem_file("tests/data/whitespace-prefix.crt").unwrap();
}

#[test]
fn different_line_endings() {
    let data = include_bytes!("data/mixed-line-endings.crt");

    // Ensure non-LF line endings are not lost by mistake, causing the test
    // to silently regress.
    let mut contained_unix_ending = false;
    let mut contained_other_ending = false;
    for byte in data.iter().copied() {
        if contained_other_ending && contained_unix_ending {
            break;
        }

        if byte == b'\n' {
            contained_unix_ending = true;
        } else if byte == b'\r' {
            contained_other_ending = true;
        }
    }
    assert!(contained_unix_ending);
    assert!(contained_other_ending);

    assert_eq!(
        CertificateDer::pem_slice_iter(data)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        CertificateDer::pem_file_iter("tests/data/mixed-line-endings.crt")
            .unwrap()
            .count(),
        4
    );
}

#[test]
fn slice_iterator() {
    let slice = b"hello\n-----BEGIN CERTIFICATE-----\naGk=\n-----END CERTIFICATE-----\ngoodbye\n";

    let mut iter = CertificateDer::pem_slice_iter(slice);
    assert_eq!(iter.remainder(), slice);
    assert_eq!(
        iter.next().unwrap().unwrap(),
        CertificateDer::from(&b"hi"[..])
    );
    assert_eq!(iter.remainder(), b"goodbye\n");
    assert!(iter.next().is_none());
}
