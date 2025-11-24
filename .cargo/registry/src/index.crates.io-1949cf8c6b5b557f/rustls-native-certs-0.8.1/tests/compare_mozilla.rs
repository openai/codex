//! This test attempts to verify that the set of 'native'
//! certificates produced by this crate is roughly similar
//! to the set of certificates in the mozilla root program
//! as expressed by the `webpki-roots` crate.
//!
//! This is, obviously, quite a heuristic test.
mod common;

use std::collections::HashMap;

use pki_types::Der;
use ring::io::der;
use serial_test::serial;
use webpki::anchor_from_trusted_cert;

fn stringify_x500name(subject: &Der<'_>) -> String {
    let mut parts = vec![];
    let mut reader = untrusted::Reader::new(subject.as_ref().into());

    while !reader.at_end() {
        let (tag, contents) = der::read_tag_and_get_value(&mut reader).unwrap();
        assert!(tag == 0x31); // sequence, constructed, context=1

        let mut inner = untrusted::Reader::new(contents);
        let pair = der::expect_tag_and_get_value(&mut inner, der::Tag::Sequence).unwrap();

        let mut pair = untrusted::Reader::new(pair);
        let oid = der::expect_tag_and_get_value(&mut pair, der::Tag::OID).unwrap();
        let (valuety, value) = der::read_tag_and_get_value(&mut pair).unwrap();

        let name = match oid.as_slice_less_safe() {
            [0x55, 0x04, 0x03] => "CN",
            [0x55, 0x04, 0x05] => "serialNumber",
            [0x55, 0x04, 0x06] => "C",
            [0x55, 0x04, 0x07] => "L",
            [0x55, 0x04, 0x08] => "ST",
            [0x55, 0x04, 0x09] => "STREET",
            [0x55, 0x04, 0x0a] => "O",
            [0x55, 0x04, 0x0b] => "OU",
            [0x55, 0x04, 0x11] => "postalCode",
            [0x55, 0x04, 0x61] => "organizationIdentifier",
            [0x09, 0x92, 0x26, 0x89, 0x93, 0xf2, 0x2c, 0x64, 0x01, 0x19] => "domainComponent",
            [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x01] => "emailAddress",
            _ => panic!("unhandled x500 attr {:?}", oid),
        };

        let str_value = match valuety {
            // PrintableString, UTF8String, TeletexString or IA5String
            0x0c | 0x13 | 0x14 | 0x16 => std::str::from_utf8(value.as_slice_less_safe()).unwrap(),
            _ => panic!("unhandled x500 value type {:?}", valuety),
        };

        parts.push(format!("{}={}", name, str_value));
    }

    parts.join(", ")
}

#[test]
fn test_does_not_have_many_roots_unknown_by_mozilla() {
    let native = rustls_native_certs::load_native_certs();
    let mozilla = webpki_roots::TLS_SERVER_ROOTS
        .iter()
        .map(|ta| (ta.subject_public_key_info.as_ref(), ta))
        .collect::<HashMap<_, _>>();

    let mut missing_in_moz_roots = 0;

    for cert in &native.certs {
        let cert = anchor_from_trusted_cert(cert).unwrap();
        if let Some(moz) = mozilla.get(cert.subject_public_key_info.as_ref()) {
            assert_eq!(cert.subject, moz.subject, "subjects differ for public key");
        } else {
            println!(
                "Native anchor {:?} is missing from mozilla set",
                stringify_x500name(&cert.subject)
            );
            missing_in_moz_roots += 1;
        }
    }

    #[cfg(windows)]
    let threshold = 2.0; // no more than 160% extra roots; windows CI vm has lots of extra roots
    #[cfg(target_os = "macos")]
    let threshold = 0.6; // macOS has a bunch of extra roots, too
    #[cfg(not(any(windows, target_os = "macos")))]
    let threshold = 0.5; // no more than 50% extra roots

    let diff = (missing_in_moz_roots as f64) / (mozilla.len() as f64);
    println!("mozilla: {:?}", mozilla.len());
    println!("native: {:?}", native.certs.len());
    println!(
        "{:?} anchors present in native set but not mozilla ({}%)",
        missing_in_moz_roots,
        diff * 100.
    );
    assert!(diff < threshold, "too many unknown roots");
}

#[test]
fn test_contains_most_roots_known_by_mozilla() {
    let native = rustls_native_certs::load_native_certs();

    let mut native_map = HashMap::new();
    for anchor in &native.certs {
        let cert = anchor_from_trusted_cert(anchor).unwrap();
        let spki = cert.subject_public_key_info.as_ref();
        native_map.insert(spki.to_owned(), anchor);
    }

    let mut missing_in_native_roots = 0;
    let mozilla = webpki_roots::TLS_SERVER_ROOTS;
    for cert in mozilla {
        if !native_map.contains_key(cert.subject_public_key_info.as_ref()) {
            println!(
                "Mozilla anchor {:?} is missing from native set",
                stringify_x500name(&cert.subject)
            );
            missing_in_native_roots += 1;
        }
    }

    #[cfg(windows)]
    let threshold = 0.95; // no more than 95% extra roots; windows misses *many* roots
    #[cfg(target_os = "macos")]
    let threshold = 0.6; // no more than 60% extra roots; macOS has a bunch of extra roots, too
    #[cfg(not(any(windows, target_os = "macos")))]
    let threshold = 0.5; // no more than 50% extra roots

    let diff = (missing_in_native_roots as f64) / (mozilla.len() as f64);
    println!("mozilla: {:?}", mozilla.len());
    println!("native: {:?}", native.certs.len());
    println!(
        "{:?} anchors present in mozilla set but not native ({}%)",
        missing_in_native_roots,
        diff * 100.
    );
    assert!(diff < threshold, "too many missing roots");
}

#[test]
#[serial]
fn util_list_certs() {
    unsafe {
        // SAFETY: safe because of #[serial]
        common::clear_env();
    }

    let native = rustls_native_certs::load_native_certs();
    for (i, cert) in native.certs.iter().enumerate() {
        let cert = anchor_from_trusted_cert(cert).unwrap();
        println!("cert[{i}] = {}", stringify_x500name(&cert.subject));
    }
}
