use std::cmp::Ordering;

use crate::asn1::{Asn1Object, Asn1OctetString, Asn1Time};
use crate::bn::{BigNum, MsbOption};
use crate::hash::MessageDigest;
use crate::nid::Nid;
use crate::pkey::{PKey, Private};
use crate::rsa::Rsa;
#[cfg(not(any(boringssl, awslc)))]
use crate::ssl::SslFiletype;
use crate::stack::Stack;
use crate::x509::extension::{
    AuthorityKeyIdentifier, BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAlternativeName,
    SubjectKeyIdentifier,
};
#[cfg(not(any(boringssl, awslc)))]
use crate::x509::store::X509Lookup;
use crate::x509::store::X509StoreBuilder;
#[cfg(any(ossl102, boringssl, libressl261, awslc))]
use crate::x509::verify::{X509VerifyFlags, X509VerifyParam};
#[cfg(any(ossl102, boringssl, awslc))]
use crate::x509::X509PurposeId;
#[cfg(any(ossl102, boringssl, libressl261, awslc))]
use crate::x509::X509PurposeRef;
#[cfg(ossl110)]
use crate::x509::{CrlReason, X509Builder};
use crate::x509::{
    CrlStatus, X509Crl, X509Extension, X509Name, X509Req, X509StoreContext, X509VerifyResult, X509,
};

#[cfg(ossl110)]
use foreign_types::ForeignType;
use hex::{self, FromHex};
#[cfg(any(ossl102, boringssl, libressl261, awslc))]
use libc::time_t;

use super::{AuthorityInformationAccess, CertificateIssuer, ReasonCode};

fn pkey() -> PKey<Private> {
    let rsa = Rsa::generate(2048).unwrap();
    PKey::from_rsa(rsa).unwrap()
}

#[test]
fn test_cert_loading() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let fingerprint = cert.digest(MessageDigest::sha1()).unwrap();

    let hash_str = "59172d9313e84459bcff27f967e79e6e9217e584";
    let hash_vec = Vec::from_hex(hash_str).unwrap();

    assert_eq!(hash_vec, &*fingerprint);
}

#[test]
fn test_debug() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let debugged = format!("{:#?}", cert);
    assert!(
        debugged.contains(r#"serial_number: "8771F7BDEE982FA5""#)
            || debugged.contains(r#"serial_number: "8771f7bdee982fa5""#)
    );
    assert!(debugged.contains(r#"signature_algorithm: sha256WithRSAEncryption"#));
    assert!(debugged.contains(r#"countryName = "AU""#));
    assert!(debugged.contains(r#"stateOrProvinceName = "Some-State""#));
    assert!(debugged.contains(r#"not_before: Aug 14 17:00:03 2016 GMT"#));
    assert!(debugged.contains(r#"not_after: Aug 12 17:00:03 2026 GMT"#));
}

#[test]
fn test_cert_issue_validity() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let not_before = cert.not_before().to_string();
    let not_after = cert.not_after().to_string();

    assert_eq!(not_before, "Aug 14 17:00:03 2016 GMT");
    assert_eq!(not_after, "Aug 12 17:00:03 2026 GMT");
}

#[test]
fn test_save_der() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();

    let der = cert.to_der().unwrap();
    assert!(!der.is_empty());
}

#[test]
fn test_subject_read_cn() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let subject = cert.subject_name();
    let cn = subject.entries_by_nid(Nid::COMMONNAME).next().unwrap();
    assert_eq!(cn.data().as_slice(), b"foobar.com")
}

#[test]
fn test_nid_values() {
    let cert = include_bytes!("../../test/nid_test_cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let subject = cert.subject_name();

    let cn = subject.entries_by_nid(Nid::COMMONNAME).next().unwrap();
    assert_eq!(cn.data().as_slice(), b"example.com");

    let email = subject
        .entries_by_nid(Nid::PKCS9_EMAILADDRESS)
        .next()
        .unwrap();
    assert_eq!(email.data().as_slice(), b"test@example.com");

    let friendly = subject.entries_by_nid(Nid::FRIENDLYNAME).next().unwrap();
    assert_eq!(&**friendly.data().as_utf8().unwrap(), "Example");
}

#[test]
fn test_nameref_iterator() {
    let cert = include_bytes!("../../test/nid_test_cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let subject = cert.subject_name();
    let mut all_entries = subject.entries();

    let email = all_entries.next().unwrap();
    assert_eq!(
        email.object().nid().as_raw(),
        Nid::PKCS9_EMAILADDRESS.as_raw()
    );
    assert_eq!(email.data().as_slice(), b"test@example.com");

    let cn = all_entries.next().unwrap();
    assert_eq!(cn.object().nid().as_raw(), Nid::COMMONNAME.as_raw());
    assert_eq!(cn.data().as_slice(), b"example.com");

    let friendly = all_entries.next().unwrap();
    assert_eq!(friendly.object().nid().as_raw(), Nid::FRIENDLYNAME.as_raw());
    assert_eq!(&**friendly.data().as_utf8().unwrap(), "Example");

    if all_entries.next().is_some() {
        panic!();
    }
}

#[test]
fn test_nid_uid_value() {
    let cert = include_bytes!("../../test/nid_uid_test_cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let subject = cert.subject_name();

    let cn = subject.entries_by_nid(Nid::USERID).next().unwrap();
    assert_eq!(cn.data().as_slice(), b"this is the userId");
}

#[test]
fn test_subject_alt_name() {
    let cert = include_bytes!("../../test/alt_name_cert.pem");
    let cert = X509::from_pem(cert).unwrap();

    let subject_alt_names = cert.subject_alt_names().unwrap();
    assert_eq!(5, subject_alt_names.len());
    assert_eq!(Some("example.com"), subject_alt_names[0].dnsname());
    assert_eq!(subject_alt_names[1].ipaddress(), Some(&[127, 0, 0, 1][..]));
    assert_eq!(
        subject_alt_names[2].ipaddress(),
        Some(&b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01"[..])
    );
    assert_eq!(Some("test@example.com"), subject_alt_names[3].email());
    assert_eq!(Some("http://www.example.com"), subject_alt_names[4].uri());
}

#[test]
#[cfg(any(ossl110, boringssl, awslc))]
fn test_retrieve_pathlen() {
    let cert = include_bytes!("../../test/root-ca.pem");
    let cert = X509::from_pem(cert).unwrap();
    assert_eq!(cert.pathlen(), None);

    let cert = include_bytes!("../../test/intermediate-ca.pem");
    let cert = X509::from_pem(cert).unwrap();
    assert_eq!(cert.pathlen(), Some(0));

    let cert = include_bytes!("../../test/alt_name_cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    assert_eq!(cert.pathlen(), None);
}

#[test]
#[cfg(any(ossl110, boringssl, awslc))]
fn test_subject_key_id() {
    let cert = include_bytes!("../../test/certv3.pem");
    let cert = X509::from_pem(cert).unwrap();

    let subject_key_id = cert.subject_key_id().unwrap();
    assert_eq!(
        subject_key_id.as_slice(),
        &b"\xB6\x73\x2F\x61\xA5\x4B\xA1\xEF\x48\x2C\x15\xB1\x9F\xF3\xDC\x34\x2F\xBC\xAC\x30"[..]
    );
}

#[test]
#[cfg(any(ossl110, boringssl, awslc))]
fn test_authority_key_id() {
    let cert = include_bytes!("../../test/certv3.pem");
    let cert = X509::from_pem(cert).unwrap();

    let authority_key_id = cert.authority_key_id().unwrap();
    assert_eq!(
        authority_key_id.as_slice(),
        &b"\x6C\xD3\xA5\x03\xAB\x0D\x5F\x2C\xC9\x8D\x8A\x9C\x88\xA7\x88\x77\xB8\x37\xFD\x9A"[..]
    );
}

#[test]
#[cfg(ossl111d)]
fn test_authority_issuer_and_serial() {
    let cert = include_bytes!("../../test/authority_key_identifier.pem");
    let cert = X509::from_pem(cert).unwrap();

    let authority_issuer = cert.authority_issuer().unwrap();
    assert_eq!(1, authority_issuer.len());
    let dn = authority_issuer[0].directory_name().unwrap();
    let mut o = dn.entries_by_nid(Nid::ORGANIZATIONNAME);
    let o = o.next().unwrap().data().as_utf8().unwrap();
    assert_eq!(o.as_bytes(), b"PyCA");
    let mut cn = dn.entries_by_nid(Nid::COMMONNAME);
    let cn = cn.next().unwrap().data().as_utf8().unwrap();
    assert_eq!(cn.as_bytes(), b"cryptography.io");

    let authority_serial = cert.authority_serial().unwrap();
    let serial = authority_serial.to_bn().unwrap();
    let expected = BigNum::from_u32(3).unwrap();
    assert_eq!(serial, expected);
}

#[test]
fn test_subject_alt_name_iter() {
    let cert = include_bytes!("../../test/alt_name_cert.pem");
    let cert = X509::from_pem(cert).unwrap();

    let subject_alt_names = cert.subject_alt_names().unwrap();
    let mut subject_alt_names_iter = subject_alt_names.iter();
    assert_eq!(
        subject_alt_names_iter.next().unwrap().dnsname(),
        Some("example.com")
    );
    assert_eq!(
        subject_alt_names_iter.next().unwrap().ipaddress(),
        Some(&[127, 0, 0, 1][..])
    );
    assert_eq!(
        subject_alt_names_iter.next().unwrap().ipaddress(),
        Some(&b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01"[..])
    );
    assert_eq!(
        subject_alt_names_iter.next().unwrap().email(),
        Some("test@example.com")
    );
    assert_eq!(
        subject_alt_names_iter.next().unwrap().uri(),
        Some("http://www.example.com")
    );
    assert!(subject_alt_names_iter.next().is_none());
}

#[test]
fn test_aia_ca_issuer() {
    // With AIA
    let cert = include_bytes!("../../test/aia_test_cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let authority_info = cert.authority_info().unwrap();
    assert_eq!(authority_info.len(), 1);
    assert_eq!(authority_info[0].method().to_string(), "CA Issuers");
    assert_eq!(
        authority_info[0].location().uri(),
        Some("http://www.example.com/cert.pem")
    );
    // Without AIA
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    assert!(cert.authority_info().is_none());
}

#[test]
fn x509_builder() {
    let pkey = pkey();

    let mut name = X509Name::builder().unwrap();
    name.append_entry_by_nid(Nid::COMMONNAME, "foobar.com")
        .unwrap();
    let name = name.build();

    let mut builder = X509::builder().unwrap();
    builder.set_version(2).unwrap();
    builder.set_subject_name(&name).unwrap();
    builder.set_issuer_name(&name).unwrap();
    builder
        .set_not_before(&Asn1Time::days_from_now(0).unwrap())
        .unwrap();
    builder
        .set_not_after(&Asn1Time::days_from_now(365).unwrap())
        .unwrap();
    builder.set_pubkey(&pkey).unwrap();

    let mut serial = BigNum::new().unwrap();
    serial.rand(128, MsbOption::MAYBE_ZERO, false).unwrap();
    builder
        .set_serial_number(&serial.to_asn1_integer().unwrap())
        .unwrap();

    let basic_constraints = BasicConstraints::new().critical().ca().build().unwrap();
    builder.append_extension(basic_constraints).unwrap();
    let key_usage = KeyUsage::new()
        .digital_signature()
        .key_encipherment()
        .build()
        .unwrap();
    builder.append_extension(key_usage).unwrap();
    let ext_key_usage = ExtendedKeyUsage::new()
        .client_auth()
        .server_auth()
        .other("2.999.1")
        .build()
        .unwrap();
    builder.append_extension(ext_key_usage).unwrap();
    let subject_key_identifier = SubjectKeyIdentifier::new()
        .build(&builder.x509v3_context(None, None))
        .unwrap();
    builder.append_extension(subject_key_identifier).unwrap();
    let authority_key_identifier = AuthorityKeyIdentifier::new()
        .keyid(true)
        .build(&builder.x509v3_context(None, None))
        .unwrap();
    builder.append_extension(authority_key_identifier).unwrap();
    let subject_alternative_name = SubjectAlternativeName::new()
        .dns("example.com")
        .build(&builder.x509v3_context(None, None))
        .unwrap();
    builder.append_extension(subject_alternative_name).unwrap();

    builder.sign(&pkey, MessageDigest::sha256()).unwrap();

    let x509 = builder.build();

    assert!(pkey.public_eq(&x509.public_key().unwrap()));
    assert!(x509.verify(&pkey).unwrap());

    let cn = x509
        .subject_name()
        .entries_by_nid(Nid::COMMONNAME)
        .next()
        .unwrap();
    assert_eq!(cn.data().as_slice(), b"foobar.com");
    assert_eq!(serial, x509.serial_number().to_bn().unwrap());
}

#[test]
// This tests `X509Extension::new`, even though its deprecated.
#[allow(deprecated)]
fn x509_extension_new() {
    assert!(X509Extension::new(None, None, "crlDistributionPoints", "section").is_err());
    assert!(X509Extension::new(None, None, "proxyCertInfo", "").is_err());
    assert!(X509Extension::new(None, None, "certificatePolicies", "").is_err());
    assert!(X509Extension::new(None, None, "subjectAltName", "dirName:section").is_err());
}

#[test]
fn x509_extension_new_from_der() {
    let ext = X509Extension::new_from_der(
        &Asn1Object::from_str("2.5.29.19").unwrap(),
        true,
        &Asn1OctetString::new_from_bytes(b"\x30\x03\x01\x01\xff").unwrap(),
    )
    .unwrap();
    assert_eq!(
        ext.to_der().unwrap(),
        b"0\x0f\x06\x03U\x1d\x13\x01\x01\xff\x04\x050\x03\x01\x01\xff"
    );
}

#[test]
fn x509_extension_to_der() {
    let builder = X509::builder().unwrap();

    for (ext, expected) in [
        (
            BasicConstraints::new().critical().ca().build().unwrap(),
            b"0\x0f\x06\x03U\x1d\x13\x01\x01\xff\x04\x050\x03\x01\x01\xff" as &[u8],
        ),
        (
            SubjectAlternativeName::new()
                .dns("example.com,DNS:example2.com")
                .build(&builder.x509v3_context(None, None))
                .unwrap(),
            b"0'\x06\x03U\x1d\x11\x04 0\x1e\x82\x1cexample.com,DNS:example2.com",
        ),
        (
            SubjectAlternativeName::new()
                .rid("1.2.3.4")
                .uri("https://example.com")
                .build(&builder.x509v3_context(None, None))
                .unwrap(),
            b"0#\x06\x03U\x1d\x11\x04\x1c0\x1a\x88\x03*\x03\x04\x86\x13https://example.com",
        ),
        (
            ExtendedKeyUsage::new()
                .server_auth()
                .other("2.999.1")
                .other("clientAuth")
                .build()
                .unwrap(),
            b"0\x22\x06\x03U\x1d%\x04\x1b0\x19\x06\x08+\x06\x01\x05\x05\x07\x03\x01\x06\x03\x887\x01\x06\x08+\x06\x01\x05\x05\x07\x03\x02",
        ),
    ] {
        assert_eq!(&ext.to_der().unwrap(), expected);
    }
}

#[test]
fn eku_invalid_other() {
    assert!(ExtendedKeyUsage::new()
        .other("1.1.1.1.1,2.2.2.2.2")
        .build()
        .is_err());
}

#[test]
fn x509_req_builder() {
    let pkey = pkey();

    let mut name = X509Name::builder().unwrap();
    name.append_entry_by_nid(Nid::COMMONNAME, "foobar.com")
        .unwrap();
    let name = name.build();

    let mut builder = X509Req::builder().unwrap();
    builder.set_version(0).unwrap();
    builder.set_subject_name(&name).unwrap();
    builder.set_pubkey(&pkey).unwrap();

    let mut extensions = Stack::new().unwrap();
    let key_usage = KeyUsage::new()
        .digital_signature()
        .key_encipherment()
        .build()
        .unwrap();
    extensions.push(key_usage).unwrap();
    let subject_alternative_name = SubjectAlternativeName::new()
        .dns("example.com")
        .build(&builder.x509v3_context(None))
        .unwrap();
    extensions.push(subject_alternative_name).unwrap();
    builder.add_extensions(&extensions).unwrap();

    builder.sign(&pkey, MessageDigest::sha256()).unwrap();

    let req = builder.build();
    assert!(req.public_key().unwrap().public_eq(&pkey));
    assert_eq!(req.extensions().unwrap().len(), extensions.len());
    assert!(req.verify(&pkey).unwrap());
}

#[test]
fn test_stack_from_pem() {
    let certs = include_bytes!("../../test/certs.pem");
    let certs = X509::stack_from_pem(certs).unwrap();

    assert_eq!(certs.len(), 2);
    assert_eq!(
        hex::encode(certs[0].digest(MessageDigest::sha1()).unwrap()),
        "59172d9313e84459bcff27f967e79e6e9217e584"
    );
    assert_eq!(
        hex::encode(certs[1].digest(MessageDigest::sha1()).unwrap()),
        "c0cbdf7cdd03c9773e5468e1f6d2da7d5cbb1875"
    );
}

#[test]
fn issued() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let ca = include_bytes!("../../test/root-ca.pem");
    let ca = X509::from_pem(ca).unwrap();

    assert_eq!(ca.issued(&cert), X509VerifyResult::OK);
    assert_ne!(cert.issued(&cert), X509VerifyResult::OK);
}

#[test]
fn signature() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let signature = cert.signature();
    assert_eq!(
        hex::encode(signature.as_slice()),
        "4af607b889790b43470442cfa551cdb8b6d0b0340d2958f76b9e3ef6ad4992230cead6842587f0ecad5\
         78e6e11a221521e940187e3d6652de14e84e82f6671f097cc47932e022add3c0cb54a26bf27fa84c107\
         4971caa6bee2e42d34a5b066c427f2d452038082b8073993399548088429de034fdd589dcfb0dd33be7\
         ebdfdf698a28d628a89568881d658151276bde333600969502c4e62e1d3470a683364dfb241f78d310a\
         89c119297df093eb36b7fd7540224f488806780305d1e79ffc938fe2275441726522ab36d88348e6c51\
         f13dcc46b5e1cdac23c974fd5ef86aa41e91c9311655090a52333bc79687c748d833595d4c5f987508f\
         e121997410d37c"
    );
    let algorithm = cert.signature_algorithm();
    assert_eq!(algorithm.object().nid(), Nid::SHA256WITHRSAENCRYPTION);
    assert_eq!(algorithm.object().to_string(), "sha256WithRSAEncryption");
}

#[test]
#[allow(clippy::redundant_clone)]
fn clone_x509() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    drop(cert.clone());
}

#[test]
fn test_verify_cert() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let ca = include_bytes!("../../test/root-ca.pem");
    let ca = X509::from_pem(ca).unwrap();
    let chain = Stack::new().unwrap();

    let mut store_bldr = X509StoreBuilder::new().unwrap();
    store_bldr.add_cert(ca).unwrap();
    let store = store_bldr.build();

    let mut context = X509StoreContext::new().unwrap();
    assert!(context
        .init(&store, &cert, &chain, |c| c.verify_cert())
        .unwrap());
    assert!(context
        .init(&store, &cert, &chain, |c| c.verify_cert())
        .unwrap());
}

#[test]
fn test_verify_fails() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let ca = include_bytes!("../../test/alt_name_cert.pem");
    let ca = X509::from_pem(ca).unwrap();
    let chain = Stack::new().unwrap();

    let mut store_bldr = X509StoreBuilder::new().unwrap();
    store_bldr.add_cert(ca).unwrap();
    let store = store_bldr.build();

    let mut context = X509StoreContext::new().unwrap();
    assert!(!context
        .init(&store, &cert, &chain, |c| c.verify_cert())
        .unwrap());
}

#[test]
#[cfg(any(ossl102, boringssl, libressl261, awslc))]
fn test_verify_fails_with_crl_flag_set_and_no_crl() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let ca = include_bytes!("../../test/root-ca.pem");
    let ca = X509::from_pem(ca).unwrap();
    let chain = Stack::new().unwrap();

    let mut store_bldr = X509StoreBuilder::new().unwrap();
    store_bldr.add_cert(ca).unwrap();
    store_bldr.set_flags(X509VerifyFlags::CRL_CHECK).unwrap();
    let store = store_bldr.build();

    let mut context = X509StoreContext::new().unwrap();
    assert_eq!(
        context
            .init(&store, &cert, &chain, |c| {
                c.verify_cert()?;
                Ok(c.error())
            })
            .unwrap()
            .error_string(),
        "unable to get certificate CRL"
    )
}

#[test]
#[cfg(any(ossl102, boringssl, libressl261, awslc))]
fn test_verify_cert_with_purpose() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let ca = include_bytes!("../../test/root-ca.pem");
    let ca = X509::from_pem(ca).unwrap();
    let chain = Stack::new().unwrap();

    let mut store_bldr = X509StoreBuilder::new().unwrap();
    let purpose_idx = X509PurposeRef::get_by_sname("sslserver")
        .expect("Getting certificate purpose 'sslserver' failed");
    let x509_purposeref =
        X509PurposeRef::from_idx(purpose_idx).expect("Getting certificate purpose failed");
    store_bldr
        .set_purpose(x509_purposeref.purpose())
        .expect("Setting certificate purpose failed");
    store_bldr.add_cert(ca).unwrap();

    let store = store_bldr.build();

    let mut context = X509StoreContext::new().unwrap();
    assert!(context
        .init(&store, &cert, &chain, |c| c.verify_cert())
        .unwrap());
}

#[test]
#[cfg(any(ossl102, boringssl, libressl261, awslc))]
fn test_verify_cert_with_wrong_purpose_fails() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let ca = include_bytes!("../../test/root-ca.pem");
    let ca = X509::from_pem(ca).unwrap();
    let chain = Stack::new().unwrap();

    let mut store_bldr = X509StoreBuilder::new().unwrap();
    let purpose_idx = X509PurposeRef::get_by_sname("timestampsign")
        .expect("Getting certificate purpose 'timestampsign' failed");
    let x509_purpose =
        X509PurposeRef::from_idx(purpose_idx).expect("Getting certificate purpose failed");
    store_bldr
        .set_purpose(x509_purpose.purpose())
        .expect("Setting certificate purpose failed");
    store_bldr.add_cert(ca).unwrap();

    let store = store_bldr.build();

    let expected_error = ffi::X509_V_ERR_INVALID_PURPOSE;
    let mut context = X509StoreContext::new().unwrap();
    assert_eq!(
        context
            .init(&store, &cert, &chain, |c| {
                c.verify_cert()?;
                Ok(c.error())
            })
            .unwrap()
            .as_raw(),
        expected_error
    )
}

#[cfg(ossl110)]
#[test]
fn x509_ref_version() {
    let mut builder = X509Builder::new().unwrap();
    let expected_version = 2;
    builder
        .set_version(expected_version)
        .expect("Failed to set certificate version");
    let cert = builder.build();
    let actual_version = cert.version();
    assert_eq!(
        expected_version, actual_version,
        "Obtained certificate version is incorrect",
    );
}

#[cfg(ossl110)]
#[test]
fn x509_ref_version_no_version_set() {
    let cert = X509Builder::new().unwrap().build();
    let actual_version = cert.version();
    assert_eq!(
        0, actual_version,
        "Default certificate version is incorrect",
    );
}

#[test]
fn test_load_crl() {
    let ca = include_bytes!("../../test/crl-ca.crt");
    let ca = X509::from_pem(ca).unwrap();

    let crl = include_bytes!("../../test/test.crl");
    let crl = X509Crl::from_der(crl).unwrap();
    assert!(crl.verify(&ca.public_key().unwrap()).unwrap());

    let cert = include_bytes!("../../test/subca.crt");
    let cert = X509::from_pem(cert).unwrap();

    let revoked = match crl.get_by_cert(&cert) {
        CrlStatus::Revoked(revoked) => revoked,
        _ => panic!("cert should be revoked"),
    };

    assert_eq!(
        revoked.serial_number().to_bn().unwrap(),
        cert.serial_number().to_bn().unwrap(),
        "revoked and cert serial numbers should match"
    );
}

#[test]
fn test_crl_entry_extensions() {
    let crl = include_bytes!("../../test/entry_extensions.crl");
    let crl = X509Crl::from_pem(crl).unwrap();

    let (critical, access_info) = crl
        .extension::<AuthorityInformationAccess>()
        .unwrap()
        .expect("Authority Information Access extension should be present");
    assert!(
        !critical,
        "Authority Information Access extension is not critical"
    );
    assert_eq!(
        access_info.len(),
        1,
        "Authority Information Access should have one entry"
    );
    assert_eq!(access_info[0].method().to_string(), "CA Issuers");
    assert_eq!(
        access_info[0].location().uri(),
        Some("http://www.example.com/ca.crt")
    );
    let revoked_certs = crl.get_revoked().unwrap();
    let entry = &revoked_certs[0];

    let (critical, issuer) = entry
        .extension::<CertificateIssuer>()
        .unwrap()
        .expect("Certificate issuer extension should be present");
    assert!(critical, "Certificate issuer extension is critical");
    assert_eq!(issuer.len(), 1, "Certificate issuer should have one entry");
    let issuer = issuer[0]
        .directory_name()
        .expect("Issuer should be a directory name");
    assert_eq!(
        format!("{:?}", issuer),
        r#"[countryName = "GB", commonName = "Test CA"]"#
    );

    // reason_code can't be inspected without ossl110
    #[allow(unused_variables)]
    let (critical, reason_code) = entry
        .extension::<ReasonCode>()
        .unwrap()
        .expect("Reason code extension should be present");
    assert!(!critical, "Reason code extension is not critical");
    #[cfg(ossl110)]
    assert_eq!(
        CrlReason::KEY_COMPROMISE,
        CrlReason::from_raw(reason_code.get_i64().unwrap() as ffi::c_int)
    );
}

#[test]
fn test_save_subject_der() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();

    let der = cert.subject_name().to_der().unwrap();
    println!("der: {:?}", der);
    assert!(!der.is_empty());
}

#[test]
fn test_load_subject_der() {
    // The subject from ../../test/cert.pem
    const SUBJECT_DER: &[u8] = &[
        48, 90, 49, 11, 48, 9, 6, 3, 85, 4, 6, 19, 2, 65, 85, 49, 19, 48, 17, 6, 3, 85, 4, 8, 12,
        10, 83, 111, 109, 101, 45, 83, 116, 97, 116, 101, 49, 33, 48, 31, 6, 3, 85, 4, 10, 12, 24,
        73, 110, 116, 101, 114, 110, 101, 116, 32, 87, 105, 100, 103, 105, 116, 115, 32, 80, 116,
        121, 32, 76, 116, 100, 49, 19, 48, 17, 6, 3, 85, 4, 3, 12, 10, 102, 111, 111, 98, 97, 114,
        46, 99, 111, 109,
    ];
    X509Name::from_der(SUBJECT_DER).unwrap();
}

#[test]
fn test_convert_to_text() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();

    const SUBSTRINGS: &[&str] = &[
        "Certificate:\n",
        "Serial Number:",
        "Signature Algorithm:",
        "Issuer: C=AU, ST=Some-State, O=Internet Widgits Pty Ltd\n",
        "Subject: C=AU, ST=Some-State, O=Internet Widgits Pty Ltd, CN=foobar.com\n",
        "Subject Public Key Info:",
    ];

    let text = String::from_utf8(cert.to_text().unwrap()).unwrap();

    for substring in SUBSTRINGS {
        assert!(
            text.contains(substring),
            "{:?} not found inside {}",
            substring,
            text
        );
    }
}

#[test]
fn test_convert_req_to_text() {
    let csr = include_bytes!("../../test/csr.pem");
    let csr = X509Req::from_pem(csr).unwrap();

    const SUBSTRINGS: &[&str] = &[
        "Certificate Request:\n",
        "Version:",
        "Subject: C=AU, ST=Some-State, O=Internet Widgits Pty Ltd, CN=foobar.com\n",
        "Subject Public Key Info:",
        "Signature Algorithm:",
    ];

    let text = String::from_utf8(csr.to_text().unwrap()).unwrap();

    for substring in SUBSTRINGS {
        assert!(
            text.contains(substring),
            "{:?} not found inside {}",
            substring,
            text
        );
    }
}

#[test]
fn test_name_cmp() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();

    let subject = cert.subject_name();
    let issuer = cert.issuer_name();
    assert_eq!(Ordering::Equal, subject.try_cmp(subject).unwrap());
    assert_eq!(Ordering::Greater, subject.try_cmp(issuer).unwrap());
}

#[test]
#[cfg(any(boringssl, ossl110, libressl270, awslc))]
fn test_name_to_owned() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let name = cert.subject_name();
    let copied_name = name.to_owned().unwrap();
    assert_eq!(Ordering::Equal, name.try_cmp(&copied_name).unwrap());
}

#[test]
#[cfg(any(ossl102, boringssl, libressl261, awslc))]
fn test_verify_param_set_time_fails_verification() {
    const TEST_T_2030: time_t = 1893456000;

    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let ca = include_bytes!("../../test/root-ca.pem");
    let ca = X509::from_pem(ca).unwrap();
    let chain = Stack::new().unwrap();

    let mut store_bldr = X509StoreBuilder::new().unwrap();
    store_bldr.add_cert(ca).unwrap();
    let mut verify_params = X509VerifyParam::new().unwrap();
    verify_params.set_time(TEST_T_2030);
    store_bldr.set_param(&verify_params).unwrap();
    let store = store_bldr.build();

    let mut context = X509StoreContext::new().unwrap();
    assert_eq!(
        context
            .init(&store, &cert, &chain, |c| {
                c.verify_cert()?;
                Ok(c.error())
            })
            .unwrap()
            .error_string(),
        "certificate has expired"
    )
}

#[test]
#[cfg(any(ossl102, boringssl, libressl261, awslc))]
fn test_verify_param_set_time() {
    const TEST_T_2020: time_t = 1577836800;

    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let ca = include_bytes!("../../test/root-ca.pem");
    let ca = X509::from_pem(ca).unwrap();
    let chain = Stack::new().unwrap();

    let mut store_bldr = X509StoreBuilder::new().unwrap();
    store_bldr.add_cert(ca).unwrap();
    let mut verify_params = X509VerifyParam::new().unwrap();
    verify_params.set_time(TEST_T_2020);
    store_bldr.set_param(&verify_params).unwrap();
    let store = store_bldr.build();

    let mut context = X509StoreContext::new().unwrap();
    assert!(context
        .init(&store, &cert, &chain, |c| c.verify_cert())
        .unwrap());
}

#[test]
#[cfg(any(ossl102, boringssl, libressl261, awslc))]
fn test_verify_param_set_depth() {
    let cert = include_bytes!("../../test/leaf.pem");
    let cert = X509::from_pem(cert).unwrap();
    let intermediate_ca = include_bytes!("../../test/intermediate-ca.pem");
    let intermediate_ca = X509::from_pem(intermediate_ca).unwrap();
    let ca = include_bytes!("../../test/root-ca.pem");
    let ca = X509::from_pem(ca).unwrap();
    let mut chain = Stack::new().unwrap();
    chain.push(intermediate_ca).unwrap();

    let mut store_bldr = X509StoreBuilder::new().unwrap();
    store_bldr.add_cert(ca).unwrap();
    let mut verify_params = X509VerifyParam::new().unwrap();
    // OpenSSL 1.1.0+ considers the root certificate to not be part of the chain, while 1.0.2 and LibreSSL do
    let expected_depth = if cfg!(any(ossl110)) { 1 } else { 2 };
    verify_params.set_depth(expected_depth);
    store_bldr.set_param(&verify_params).unwrap();
    let store = store_bldr.build();

    let mut context = X509StoreContext::new().unwrap();
    assert!(context
        .init(&store, &cert, &chain, |c| c.verify_cert())
        .unwrap());
}

#[test]
#[cfg(any(ossl102, boringssl, libressl261, awslc))]
#[allow(clippy::bool_to_int_with_if)]
fn test_verify_param_set_depth_fails_verification() {
    let cert = include_bytes!("../../test/leaf.pem");
    let cert = X509::from_pem(cert).unwrap();
    let intermediate_ca = include_bytes!("../../test/intermediate-ca.pem");
    let intermediate_ca = X509::from_pem(intermediate_ca).unwrap();
    let ca = include_bytes!("../../test/root-ca.pem");
    let ca = X509::from_pem(ca).unwrap();
    let mut chain = Stack::new().unwrap();
    chain.push(intermediate_ca).unwrap();

    let mut store_bldr = X509StoreBuilder::new().unwrap();
    store_bldr.add_cert(ca).unwrap();
    let mut verify_params = X509VerifyParam::new().unwrap();
    // OpenSSL 1.1.0+ considers the root certificate to not be part of the chain, while 1.0.2 and LibreSSL do
    let expected_depth = if cfg!(any(ossl110, boringssl, awslc)) {
        0
    } else {
        1
    };
    verify_params.set_depth(expected_depth);
    store_bldr.set_param(&verify_params).unwrap();
    let store = store_bldr.build();

    // OpenSSL 1.1.0+ added support for X509_V_ERR_CERT_CHAIN_TOO_LONG, while 1.0.2 simply ignores the intermediate
    let expected_error = if cfg!(any(ossl110, libressl261)) {
        "certificate chain too long"
    } else {
        "unable to get local issuer certificate"
    };

    let mut context = X509StoreContext::new().unwrap();
    assert_eq!(
        context
            .init(&store, &cert, &chain, |c| {
                c.verify_cert()?;
                Ok(c.error())
            })
            .unwrap()
            .error_string(),
        expected_error
    )
}

#[test]
#[cfg(not(any(boringssl, awslc)))]
fn test_load_cert_file() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let chain = Stack::new().unwrap();

    let mut store_bldr = X509StoreBuilder::new().unwrap();
    let lookup = store_bldr.add_lookup(X509Lookup::file()).unwrap();
    lookup
        .load_cert_file("test/root-ca.pem", SslFiletype::PEM)
        .unwrap();
    let store = store_bldr.build();

    let mut context = X509StoreContext::new().unwrap();
    assert!(context
        .init(&store, &cert, &chain, |c| c.verify_cert())
        .unwrap());
}

#[test]
#[cfg(ossl110)]
fn test_verify_param_auth_level() {
    let mut param = X509VerifyParam::new().unwrap();
    let auth_lvl = 2;
    let auth_lvl_default = -1;

    assert_eq!(param.auth_level(), auth_lvl_default);

    param.set_auth_level(auth_lvl);
    assert_eq!(param.auth_level(), auth_lvl);
}

#[test]
#[cfg(any(ossl102, boringssl, awslc))]
fn test_set_purpose() {
    let cert = include_bytes!("../../test/leaf.pem");
    let cert = X509::from_pem(cert).unwrap();
    let intermediate_ca = include_bytes!("../../test/intermediate-ca.pem");
    let intermediate_ca = X509::from_pem(intermediate_ca).unwrap();
    let ca = include_bytes!("../../test/root-ca.pem");
    let ca = X509::from_pem(ca).unwrap();
    let mut chain = Stack::new().unwrap();
    chain.push(intermediate_ca).unwrap();

    let mut store_bldr = X509StoreBuilder::new().unwrap();
    store_bldr.add_cert(ca).unwrap();
    let mut verify_params = X509VerifyParam::new().unwrap();
    verify_params.set_purpose(X509PurposeId::ANY).unwrap();
    store_bldr.set_param(&verify_params).unwrap();
    let store = store_bldr.build();
    let mut context = X509StoreContext::new().unwrap();

    assert!(context
        .init(&store, &cert, &chain, |c| c.verify_cert())
        .unwrap());
}

#[test]
#[cfg(any(ossl102, boringssl, awslc))]
fn test_set_purpose_fails_verification() {
    let cert = include_bytes!("../../test/leaf.pem");
    let cert = X509::from_pem(cert).unwrap();
    let intermediate_ca = include_bytes!("../../test/intermediate-ca.pem");
    let intermediate_ca = X509::from_pem(intermediate_ca).unwrap();
    let ca = include_bytes!("../../test/root-ca.pem");
    let ca = X509::from_pem(ca).unwrap();
    let mut chain = Stack::new().unwrap();
    chain.push(intermediate_ca).unwrap();

    let mut store_bldr = X509StoreBuilder::new().unwrap();
    store_bldr.add_cert(ca).unwrap();
    let mut verify_params = X509VerifyParam::new().unwrap();
    verify_params
        .set_purpose(X509PurposeId::TIMESTAMP_SIGN)
        .unwrap();
    store_bldr.set_param(&verify_params).unwrap();
    let store = store_bldr.build();

    let expected_error = ffi::X509_V_ERR_INVALID_PURPOSE;
    let mut context = X509StoreContext::new().unwrap();
    assert_eq!(
        context
            .init(&store, &cert, &chain, |c| {
                c.verify_cert()?;
                Ok(c.error())
            })
            .unwrap()
            .as_raw(),
        expected_error
    )
}

#[test]
#[cfg(any(ossl101, libressl350))]
fn test_add_name_entry() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    let inp_name = cert.subject_name().entries().next().unwrap();

    let mut names = X509Name::builder().unwrap();
    names.append_entry(inp_name).unwrap();
    let names = names.build();

    let mut entries = names.entries();
    let outp_name = entries.next().unwrap();
    assert_eq!(outp_name.object().nid(), inp_name.object().nid());
    assert_eq!(outp_name.data().as_slice(), inp_name.data().as_slice());
    assert!(entries.next().is_none());
}

#[test]
#[cfg(not(any(boringssl, awslc)))]
fn test_load_crl_file_fail() {
    let mut store_bldr = X509StoreBuilder::new().unwrap();
    let lookup = store_bldr.add_lookup(X509Lookup::file()).unwrap();
    let res = lookup.load_crl_file("test/root-ca.pem", SslFiletype::PEM);
    assert!(res.is_err());
}

#[cfg(ossl110)]
fn ipaddress_as_subject_alternative_name_is_formatted_in_debug<T>(expected_ip: T)
where
    T: Into<std::net::IpAddr>,
{
    let expected_ip = format!("{:?}", expected_ip.into());
    let mut builder = X509Builder::new().unwrap();
    let san = SubjectAlternativeName::new()
        .ip(&expected_ip)
        .build(&builder.x509v3_context(None, None))
        .unwrap();
    builder.append_extension(san).unwrap();
    let cert = builder.build();
    let actual_ip = cert
        .subject_alt_names()
        .into_iter()
        .flatten()
        .map(|n| format!("{:?}", *n))
        .next()
        .unwrap();
    assert_eq!(actual_ip, expected_ip);
}

#[cfg(ossl110)]
#[test]
fn ipv4_as_subject_alternative_name_is_formatted_in_debug() {
    ipaddress_as_subject_alternative_name_is_formatted_in_debug([8u8, 8, 8, 128]);
}

#[cfg(ossl110)]
#[test]
fn ipv6_as_subject_alternative_name_is_formatted_in_debug() {
    ipaddress_as_subject_alternative_name_is_formatted_in_debug([
        8u8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 128,
    ]);
}

#[cfg(ossl110)]
#[test]
fn other_name_as_subject_alternative_name() {
    let oid = Asn1Object::from_str("1.3.6.1.5.5.7.8.11").unwrap();
    // this is the hex representation of "test" encoded as a ia5string
    let content = [0x16, 0x04, 0x74, 0x65, 0x73, 0x74];

    let mut builder = X509Builder::new().unwrap();
    let san = SubjectAlternativeName::new()
        .other_name2(oid, &content)
        .build(&builder.x509v3_context(None, None))
        .unwrap();
    builder.append_extension(san).unwrap();
    let cert = builder.build();
    let general_name = cert
        .subject_alt_names()
        .into_iter()
        .flatten()
        .next()
        .unwrap();
    unsafe {
        assert_eq!((*general_name.as_ptr()).type_, 0);
    }
}

#[test]
fn test_dist_point() {
    let cert = include_bytes!("../../test/certv3.pem");
    let cert = X509::from_pem(cert).unwrap();

    let dps = cert.crl_distribution_points().unwrap();
    let dp = dps.get(0).unwrap();
    let dp_nm = dp.distpoint().unwrap();
    let dp_gns = dp_nm.fullname().unwrap();
    let dp_gn = dp_gns.get(0).unwrap();
    assert_eq!(dp_gn.uri().unwrap(), "http://example.com/crl.pem");

    let dp = dps.get(1).unwrap();
    let dp_nm = dp.distpoint().unwrap();
    let dp_gns = dp_nm.fullname().unwrap();
    let dp_gn = dp_gns.get(0).unwrap();
    assert_eq!(dp_gn.uri().unwrap(), "http://example.com/crl2.pem");
    assert!(dps.get(2).is_none())
}

#[test]
fn test_dist_point_null() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();
    assert!(cert.crl_distribution_points().is_none());
}

#[test]
#[cfg(ossl300)]
fn test_store_all_certificates() {
    let cert = include_bytes!("../../test/cert.pem");
    let cert = X509::from_pem(cert).unwrap();

    let store = {
        let mut b = X509StoreBuilder::new().unwrap();
        b.add_cert(cert).unwrap();
        b.build()
    };

    assert_eq!(store.all_certificates().len(), 1);
}
