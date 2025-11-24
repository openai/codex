use std::prelude::v1::*;
use std::{format, println, vec};

use pki_types::{CertificateDer, DnsName};

use super::base::{Payload, PayloadU8, PayloadU16, PayloadU24};
use super::codec::{Codec, Reader, put_u16};
use super::enums::{
    ClientCertificateType, Compression, ECCurveType, ExtensionType, KeyUpdateRequest, NamedGroup,
};
use super::handshake::{
    CertificateChain, CertificateEntry, CertificateExtensions, CertificatePayloadTls13,
    CertificateRequestExtensions, CertificateRequestPayload, CertificateRequestPayloadTls13,
    CertificateStatus, CertificateStatusRequest, ClientExtensions, ClientHelloPayload,
    ClientSessionTicket, CompressedCertificatePayload, DistinguishedName, EcParameters,
    EncryptedClientHello, HandshakeMessagePayload, HandshakePayload, HelloRetryRequest,
    HelloRetryRequestExtensions, KeyShareEntry, NewSessionTicketExtensions,
    NewSessionTicketPayload, NewSessionTicketPayloadTls13, PresharedKeyBinder,
    PresharedKeyIdentity, PresharedKeyOffer, ProtocolName, PskKeyExchangeModes, Random,
    ServerDhParams, ServerEcdhParams, ServerEncryptedClientHello, ServerExtensions,
    ServerHelloPayload, ServerKeyExchange, ServerKeyExchangeParams, ServerKeyExchangePayload,
    ServerNamePayload, SessionId, SingleProtocolName, SupportedEcPointFormats,
    SupportedProtocolVersions,
};
use crate::enums::{
    CertificateCompressionAlgorithm, CertificateType, CipherSuite, HandshakeType, ProtocolVersion,
    SignatureScheme,
};
use crate::error::InvalidMessage;
use crate::sync::Arc;
use crate::verify::DigitallySignedStruct;

#[test]
fn rejects_short_random() {
    let bytes = [0x01; 31];
    let mut rd = Reader::init(&bytes);
    assert!(Random::read(&mut rd).is_err());
}

#[test]
fn reads_random() {
    let bytes = [0x01; 32];
    let mut rd = Reader::init(&bytes);
    let rnd = Random::read(&mut rd).unwrap();
    println!("{rnd:?}");

    assert!(!rd.any_left());
}

#[test]
fn debug_random() {
    assert_eq!(
        "0101010101010101010101010101010101010101010101010101010101010101",
        format!("{:?}", Random::from([1; 32]))
    );
}

#[test]
fn rejects_truncated_session_id() {
    let bytes = [32; 32];
    let mut rd = Reader::init(&bytes);
    assert!(SessionId::read(&mut rd).is_err());
}

#[test]
fn rejects_session_id_with_bad_length() {
    let bytes = [33; 33];
    let mut rd = Reader::init(&bytes);
    assert!(SessionId::read(&mut rd).is_err());
}

#[test]
fn session_id_with_different_lengths_are_unequal() {
    let a = SessionId::read(&mut Reader::init(&[1u8, 1])).unwrap();
    let b = SessionId::read(&mut Reader::init(&[2u8, 1, 2])).unwrap();
    assert_ne!(a, b);
}

#[test]
fn accepts_short_session_id() {
    let bytes = [1; 2];
    let mut rd = Reader::init(&bytes);
    let sess = SessionId::read(&mut rd).unwrap();
    println!("{sess:?}");

    #[cfg(feature = "tls12")]
    assert!(!sess.is_empty());
    assert_ne!(sess, SessionId::empty());
    assert!(!rd.any_left());
}

#[test]
fn accepts_empty_session_id() {
    let bytes = [0; 1];
    let mut rd = Reader::init(&bytes);
    let sess = SessionId::read(&mut rd).unwrap();
    println!("{sess:?}");

    #[cfg(feature = "tls12")]
    assert!(sess.is_empty());
    assert_eq!(sess, SessionId::empty());
    assert!(!rd.any_left());
}

#[test]
fn debug_session_id() {
    let bytes = [
        32, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1, 1,
    ];
    let mut rd = Reader::init(&bytes);
    let sess = SessionId::read(&mut rd).unwrap();
    assert_eq!(
        "0101010101010101010101010101010101010101010101010101010101010101",
        format!("{sess:?}")
    );
}

#[test]
fn refuses_client_exts_with_unparsed_bytes() {
    let bytes = [0x00u8, 0x08, 0x00, 0x0b, 0x00, 0x04, 0x02, 0xf8, 0x01, 0x02];
    assert_eq!(
        ClientExtensions::read_bytes(&bytes).unwrap_err(),
        InvalidMessage::TrailingData("ClientExtensions")
    );
}

#[test]
fn refuses_server_ext_with_unparsed_bytes() {
    let bytes = [0x00u8, 0x08, 0x00, 0x0b, 0x00, 0x04, 0x02, 0xf8, 0x01, 0x02];
    assert_eq!(
        ServerExtensions::read_bytes(&bytes).unwrap_err(),
        InvalidMessage::TrailingData("ServerExtensions")
    );
}

#[test]
fn refuses_certificate_ext_with_unparsed_bytes() {
    let bytes = [
        0x00u8, 0x09, 0x00, 0x05, 0x00, 0x05, 0x01, 0x00, 0x00, 0x00, 0x01,
    ];
    assert_eq!(
        CertificateExtensions::read_bytes(&bytes).unwrap_err(),
        InvalidMessage::TrailingData("CertificateExtensions")
    );
}

#[test]
fn refuses_certificate_ext_with_unknown_type() {
    let bytes = [0x00u8, 0x08, 0x00, 0x05, 0x00, 0x03, 0x99, 0x00, 0x00, 0x00];
    assert_eq!(
        CertificateExtensions::read_bytes(&bytes).unwrap_err(),
        InvalidMessage::InvalidCertificateStatusType
    );
}

#[test]
fn refuses_certificate_req_ext_with_unparsed_bytes() {
    let bytes = [
        0x00u8, 0x09, 0x00, 0x0d, 0x00, 0x05, 0x00, 0x02, 0x01, 0x02, 0xff,
    ];
    assert_eq!(
        CertificateRequestExtensions::read_bytes(&bytes).unwrap_err(),
        InvalidMessage::TrailingData("CertificateRequestExtensions")
    );
}

#[test]
fn refuses_certificate_req_ext_with_duplicate() {
    let bytes = [0x00u8, 0x08, 0x00, 0x99, 0x00, 0x00, 0x00, 0x99, 0x00, 0x00];
    assert_eq!(
        CertificateRequestExtensions::read_bytes(&bytes).unwrap_err(),
        InvalidMessage::DuplicateExtension(0x0099)
    );
}

#[test]
fn refuses_new_session_ticket_ext_with_unparsed_bytes() {
    let bytes = [
        0x00u8, 0x09, 0x00, 0x2a, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x01,
    ];
    assert_eq!(
        NewSessionTicketExtensions::read_bytes(&bytes).unwrap_err(),
        InvalidMessage::TrailingData("NewSessionTicketExtensions")
    );
}

#[test]
fn refuses_new_session_ticket_ext_with_duplicate_extension() {
    let bytes = [0x00u8, 0x08, 0x00, 0x99, 0x00, 0x00, 0x00, 0x99, 0x00, 0x00];
    assert_eq!(
        NewSessionTicketExtensions::read_bytes(&bytes).unwrap_err(),
        InvalidMessage::DuplicateExtension(0x0099)
    );
}

#[test]
fn rejects_truncated_sni() {
    let bytes = [0, 1, 0];
    assert!(ServerNamePayload::read(&mut Reader::init(&bytes)).is_err());

    let bytes = [0, 2, 0, 1];
    assert!(ServerNamePayload::read(&mut Reader::init(&bytes)).is_err());

    let bytes = [0, 3, 0, 1, 0];
    assert!(ServerNamePayload::read(&mut Reader::init(&bytes)).is_err());

    let bytes = [0, 4, 0, 2, 0, 0];
    assert!(ServerNamePayload::read(&mut Reader::init(&bytes)).is_err());

    let bytes = [0, 5, 0, 3, 0, 0, 0];
    assert!(ServerNamePayload::read(&mut Reader::init(&bytes)).is_err());

    let bytes = [0, 5, 0, 3, 0, 0, 1];
    assert!(ServerNamePayload::read(&mut Reader::init(&bytes)).is_err());

    let bytes = [0, 6, 0, 4, 0, 0, 2, 0x68];
    assert!(ServerNamePayload::read(&mut Reader::init(&bytes)).is_err());
}

#[test]
fn rejects_empty_sni_extension() {
    assert_eq!(
        ClientExtensions::read_bytes(&[0, 6, 0, 0, 0, 2, 0, 0]).unwrap_err(),
        InvalidMessage::IllegalEmptyList("ServerNames")
    );
}

#[test]
fn rejects_duplicate_names_in_sni_extension() {
    assert_eq!(
        ClientExtensions::read_bytes(&[0, 14, 0, 0, 0, 10, 0, 8, 0, 0, 1, b'a', 0, 0, 1, b'b',])
            .unwrap_err(),
        InvalidMessage::InvalidServerName
    );
}

#[test]
fn can_round_trip_psk_identity() {
    let bytes = [0, 1, 0x99, 0x11, 0x22, 0x33, 0x44];
    let psk_id = PresharedKeyIdentity::read(&mut Reader::init(&bytes)).unwrap();
    println!("{psk_id:?}");
    assert_eq!(psk_id.obfuscated_ticket_age, 0x11223344);
    assert_eq!(psk_id.get_encoding(), bytes.to_vec());

    let bytes = [0, 5, 0x1, 0x2, 0x3, 0x4, 0x5, 0x11, 0x22, 0x33, 0x44];
    let psk_id = PresharedKeyIdentity::read(&mut Reader::init(&bytes)).unwrap();
    println!("{psk_id:?}");
    assert_eq!(psk_id.identity.0, vec![0x1, 0x2, 0x3, 0x4, 0x5]);
    assert_eq!(psk_id.obfuscated_ticket_age, 0x11223344);
    assert_eq!(psk_id.get_encoding(), bytes.to_vec());
}

#[test]
fn can_round_trip_psk_offer() {
    let bytes = [
        0, 7, 0, 1, 0x99, 0x11, 0x22, 0x33, 0x44, 0, 4, 3, 0x01, 0x02, 0x3,
    ];
    let psko = PresharedKeyOffer::read(&mut Reader::init(&bytes)).unwrap();
    println!("{psko:?}");

    assert_eq!(psko.identities.len(), 1);
    assert_eq!(psko.identities[0].identity.0, vec![0x99]);
    assert_eq!(psko.identities[0].obfuscated_ticket_age, 0x11223344);
    assert_eq!(psko.binders.len(), 1);
    assert_eq!(psko.binders[0].as_ref(), &[1, 2, 3]);
    assert_eq!(psko.get_encoding(), bytes.to_vec());
}

#[test]
fn can_round_trip_cert_status_req_for_ocsp() {
    let ext = CertificateStatusRequest::build_ocsp();
    println!("{ext:?}");

    let bytes = [
        0, 11, 1, // OCSP
        0, 5, 0, 3, 0, 1, 1, 0, 1, 2,
    ];

    let csr = CertificateStatusRequest::read(&mut Reader::init(&bytes)).unwrap();
    println!("{csr:?}");
    assert_eq!(csr.get_encoding(), bytes.to_vec());
}

#[test]
fn can_round_trip_cert_status_req_for_other() {
    let bytes = [
        0, 5, 2, // !OCSP
        1, 2, 3, 4,
    ];

    let csr = CertificateStatusRequest::read(&mut Reader::init(&bytes)).unwrap();
    println!("{csr:?}");
    assert_eq!(csr.get_encoding(), bytes.to_vec());
}

#[test]
fn can_print_all_client_extensions() {
    println!("client hello {:?}", sample_client_hello_payload());
}

#[test]
fn can_clone_all_client_extensions() {
    let exts = sample_client_hello_payload().extensions;
    let exts2 = exts.clone();
    println!("{exts:?}, {exts2:?}");
}

#[test]
fn client_extensions_basics() {
    let src = ClientExtensions {
        early_data_request: Some(()),
        ..Default::default()
    };
    let mut target = ClientExtensions::default();

    assert_eq!(src.collect_used(), vec![ExtensionType::EarlyData]);
    assert_eq!(target.collect_used(), vec![]);

    target.clone_one(&src, ExtensionType::EarlyData);
    assert_eq!(target.collect_used(), vec![ExtensionType::EarlyData]);
}

#[test]
fn client_extensions_empty() {
    // both sides of empty-encoding branch
    assert_eq!(ClientExtensions::default().get_encoding(), Vec::<u8>::new());
    assert_eq!(
        ClientExtensions::read_bytes(&[])
            .unwrap()
            .collect_used(),
        vec![]
    );

    let early_data = b"\x00\x04\x00\x2a\x00\x00";
    assert_eq!(
        ClientExtensions {
            early_data_request: Some(()),
            ..Default::default()
        }
        .get_encoding(),
        early_data
    );
    assert_eq!(
        ClientExtensions::read_bytes(early_data)
            .unwrap()
            .collect_used(),
        vec![ExtensionType::EarlyData]
    );
}

#[test]
fn client_extensions_decode_checks_duplicates() {
    // base
    ClientExtensions::read_bytes(b"\x00\x04\x00\x2a\x00\x00").unwrap();

    // duplicate known
    assert_eq!(
        ClientExtensions::read_bytes(b"\x00\x08\x00\x2a\x00\x00\x00\x2a\x00\x00").unwrap_err(),
        InvalidMessage::DuplicateExtension(0x002a)
    );

    // duplicate unknown
    assert_eq!(
        ClientExtensions::read_bytes(b"\x00\x08\xff\xff\x00\x00\xff\xff\x00\x00").unwrap_err(),
        InvalidMessage::DuplicateExtension(0xffff)
    );
}

#[test]
fn client_extensions_ordering() {
    // the important thing here is that PSK requests come last,
    // ECH requests come second to last, and order of other extensions
    // do vary.

    let psk_offer = PresharedKeyOffer {
        identities: vec![],
        binders: vec![],
    };

    let psk_and_ech = ClientExtensions {
        early_data_request: Some(()),
        extended_master_secret_request: Some(()),
        preshared_key_offer: Some(psk_offer.clone()),
        encrypted_client_hello: Some(EncryptedClientHello::Inner),
        ..Default::default()
    };

    let psk_and_ech_with_contiguous = ClientExtensions {
        contiguous_extensions: vec![ExtensionType::ExtendedMasterSecret],
        ..psk_and_ech.clone()
    };

    let ech = ClientExtensions {
        early_data_request: Some(()),
        extended_master_secret_request: Some(()),
        encrypted_client_hello: Some(EncryptedClientHello::Inner),
        ..Default::default()
    };

    let psk = ClientExtensions {
        early_data_request: Some(()),
        extended_master_secret_request: Some(()),
        preshared_key_offer: Some(psk_offer),
        ..Default::default()
    };

    let neither = ClientExtensions {
        early_data_request: Some(()),
        extended_master_secret_request: Some(()),
        ..Default::default()
    };

    fn encoding_with_order(order_seed: u16, exts: &ClientExtensions<'_>) -> Vec<u8> {
        let mut e = exts.clone();
        e.order_seed = order_seed;
        e.get_encoding()
    }

    assert_ne!(
        encoding_with_order(0, &psk_and_ech),
        encoding_with_order(1, &psk_and_ech)
    );
    assert_eq!(
        encoding_with_order(0, &psk_and_ech_with_contiguous),
        encoding_with_order(1, &psk_and_ech_with_contiguous)
    );
    assert_ne!(encoding_with_order(0, &ech), encoding_with_order(1, &ech));
    assert_ne!(encoding_with_order(0, &psk), encoding_with_order(1, &psk));
    assert_ne!(
        encoding_with_order(0, &neither),
        encoding_with_order(1, &neither)
    );

    // check order invariants hold for all seeds
    for seed in 0..=0xffff {
        // must end with ECH and then PSK
        assert!(encoding_with_order(seed, &psk_and_ech).ends_with(
            b"\xfe\x0d\x00\x01\x01\
              \x00\x29\x00\x04\x00\x00\x00\x00"
        ));

        // must end with EMS, then ECH and then PSK
        assert!(
            encoding_with_order(seed, &psk_and_ech_with_contiguous).ends_with(
                b"\x00\x17\x00\x00\
                  \xfe\x0d\x00\x01\x01\
                  \x00\x29\x00\x04\x00\x00\x00\x00"
            )
        );

        // just PSK
        assert!(encoding_with_order(seed, &psk).ends_with(b"\x00\x29\x00\x04\x00\x00\x00\x00"));

        // just ECH
        assert!(encoding_with_order(seed, &ech).ends_with(b"\xfe\x0d\x00\x01\x01"));
    }
}

#[test]
fn test_truncated_psk_offer() {
    let ext = PresharedKeyOffer {
        identities: vec![PresharedKeyIdentity::new(vec![3, 4, 5], 123456)],
        binders: vec![PresharedKeyBinder::from(vec![1, 2, 3])],
    };

    let mut enc = ext.get_encoding();
    println!("testing {ext:?} enc {enc:?}");
    for l in 0..enc.len() {
        if l == 9 {
            continue;
        }
        put_u16(l as u16, &mut enc);
        let rc = PresharedKeyOffer::read_bytes(&enc);
        assert!(rc.is_err());
    }
}

#[test]
fn test_truncated_client_hello_is_detected() {
    let ch = sample_client_hello_payload();
    let enc = ch.get_encoding();
    println!("testing {ch:?} enc {enc:?}");

    for l in 0..enc.len() {
        println!("len {:?} enc {:?}", l, &enc[..l]);
        if l == 41 {
            continue; // where extensions are empty
        }
        assert!(ClientHelloPayload::read_bytes(&enc[..l]).is_err());
    }
}

#[test]
fn test_truncated_client_extension_is_detected() {
    let chp = sample_client_hello_payload();

    let enc = chp.extensions.get_encoding();
    println!("testing enc {enc:?}");

    // "outer" truncation, i.e., where the extension-level length is longer than
    // the input
    for l in 1..enc.len() {
        assert!(ClientExtensions::read_bytes(&enc[..l]).is_err());
    }
}

#[test]
fn test_truncated_hello_retry_extension_is_detected() {
    let hrr = sample_hello_retry_request();

    let mut enc = hrr.extensions.get_encoding();
    println!("testing enc {enc:?}");

    // "outer" truncation, i.e., where the extension-level length is longer than
    // the input
    for l in 0..enc.len() {
        assert!(HelloRetryRequestExtensions::read_bytes(&enc[..l]).is_err());
    }

    // "inner" truncation, where the extension-level length agrees with the input
    // length, but isn't long enough for the type of extension
    for l in 0..(enc.len() - 4) {
        put_u16(l as u16, &mut enc);
        println!("  encoding {enc:?} len {l:?}");
        assert!(HelloRetryRequestExtensions::read_bytes(&enc).is_err());
    }
}

#[test]
fn test_truncated_server_extension_is_detected() {
    let shp = sample_server_hello_payload();

    let mut enc = shp.extensions.get_encoding();
    println!("testing enc {enc:?}");

    // "outer" truncation, i.e., where the extension-level length is longer than
    // the input
    for l in 0..enc.len() {
        assert!(ServerExtensions::read_bytes(&enc[..l]).is_err());
    }

    // "inner" truncation, where the extension-level length agrees with the input
    // length, but isn't long enough for the type of extension
    for l in 0..(enc.len() - 4) {
        put_u16(l as u16, &mut enc[..2]);
        println!("  encoding {enc:?} len {l:?}");
        assert!(ServerExtensions::read_bytes(&enc).is_err());
    }
}

#[test]
fn can_print_all_server_extensions() {
    println!("server hello {:?}", sample_server_hello_payload());
}

#[test]
fn can_clone_all_server_extensions() {
    let exts = sample_server_hello_payload().extensions;
    let exts2 = exts.clone();
    println!("{exts:?}, {exts2:?}");
}

#[test]
fn can_round_trip_all_tls12_handshake_payloads() {
    for hm in all_tls12_handshake_payloads().iter() {
        println!("{:?}", hm.0.handshake_type());
        let bytes = hm.get_encoding();
        let mut rd = Reader::init(&bytes);
        let other = HandshakeMessagePayload::read(&mut rd).unwrap();
        assert!(!rd.any_left());
        assert_eq!(hm.get_encoding(), other.get_encoding());

        println!("{hm:?}");
        println!("{other:?}");
    }
}

#[test]
fn can_into_owned_all_tls12_handshake_payloads() {
    for hm in all_tls12_handshake_payloads().drain(..) {
        let enc = hm.get_encoding();
        let debug = format!("{hm:?}");
        let other = hm.into_owned();
        assert_eq!(enc, other.get_encoding());
        assert_eq!(debug, format!("{other:?}"));
    }
}

#[test]
fn can_detect_truncation_of_all_tls12_handshake_payloads() {
    for hm in all_tls12_handshake_payloads().iter() {
        let mut enc = hm.get_encoding();
        println!("test {hm:?} enc {enc:?}");

        // outer truncation
        for l in 0..enc.len() {
            assert!(HandshakeMessagePayload::read_bytes(&enc[..l]).is_err())
        }

        // inner truncation
        for l in 0..enc.len() - 4 {
            put_u24(l as u32, &mut enc[1..]);
            println!("  check len {l:?} enc {enc:?}");

            match (hm.0.handshake_type(), l) {
                (HandshakeType::ClientHello, 41)
                | (HandshakeType::ServerHello, 38)
                | (HandshakeType::ServerKeyExchange, _)
                | (HandshakeType::ClientKeyExchange, _)
                | (HandshakeType::Finished, _)
                | (HandshakeType::Unknown(_), _) => continue,
                _ => {}
            };

            assert!(
                HandshakeMessagePayload::read_version(
                    &mut Reader::init(&enc),
                    ProtocolVersion::TLSv1_2
                )
                .is_err()
            );
            assert!(HandshakeMessagePayload::read_bytes(&enc).is_err());
        }
    }
}

#[test]
fn can_round_trip_all_tls13_handshake_payloads() {
    for hm in all_tls13_handshake_payloads().iter() {
        println!("{:?}", hm.0.handshake_type());
        let bytes = hm.get_encoding();
        let mut rd = Reader::init(&bytes);

        let other =
            HandshakeMessagePayload::read_version(&mut rd, ProtocolVersion::TLSv1_3).unwrap();
        assert!(!rd.any_left());
        assert_eq!(hm.get_encoding(), other.get_encoding());

        println!("{hm:?}");
        println!("{other:?}");
    }
}

#[test]
fn can_into_owned_all_tls13_handshake_payloads() {
    for hm in all_tls13_handshake_payloads().drain(..) {
        let enc = hm.get_encoding();
        let debug = format!("{hm:?}");
        let other = hm.into_owned();
        assert_eq!(enc, other.get_encoding());
        assert_eq!(debug, format!("{other:?}"));
    }
}

#[test]
fn can_detect_truncation_of_all_tls13_handshake_payloads() {
    for hm in all_tls13_handshake_payloads().iter() {
        let mut enc = hm.get_encoding();
        println!("test {hm:?} enc {enc:?}");

        // outer truncation
        for l in 0..enc.len() {
            assert!(HandshakeMessagePayload::read_bytes(&enc[..l]).is_err())
        }

        // inner truncation
        for l in 0..enc.len() - 4 {
            put_u24(l as u32, &mut enc[1..]);
            println!("  check len {l:?} enc {enc:?}");

            match (hm.0.handshake_type(), l) {
                (HandshakeType::ClientHello, 41)
                | (HandshakeType::ServerHello, 38)
                | (HandshakeType::ServerKeyExchange, _)
                | (HandshakeType::ClientKeyExchange, _)
                | (HandshakeType::Finished, _)
                | (HandshakeType::Unknown(_), _) => continue,
                _ => {}
            };

            assert!(
                HandshakeMessagePayload::read_version(
                    &mut Reader::init(&enc),
                    ProtocolVersion::TLSv1_3
                )
                .is_err()
            );
        }
    }
}

fn put_u24(u: u32, b: &mut [u8]) {
    b[0] = (u >> 16) as u8;
    b[1] = (u >> 8) as u8;
    b[2] = u as u8;
}

#[test]
fn cannot_read_message_hash_from_network() {
    let mh = HandshakeMessagePayload(HandshakePayload::MessageHash(Payload::new(vec![1, 2, 3])));
    println!("mh {mh:?}");
    let enc = mh.get_encoding();
    assert!(HandshakeMessagePayload::read_bytes(&enc).is_err());
}

#[test]
fn cannot_decode_huge_certificate() {
    let mut buf = [0u8; 65 * 1024];
    // exactly 64KB decodes fine
    buf[0] = 0x0b;
    buf[1] = 0x01;
    buf[2] = 0x00;
    buf[3] = 0x03;
    buf[4] = 0x01;
    buf[5] = 0x00;
    buf[6] = 0x00;
    buf[7] = 0x00;
    buf[8] = 0xff;
    buf[9] = 0xfd;
    HandshakeMessagePayload::read_bytes(&buf[..0x10000 + 7]).unwrap();

    // however 64KB + 1 byte does not
    buf[1] = 0x01;
    buf[2] = 0x00;
    buf[3] = 0x04;
    buf[4] = 0x01;
    buf[5] = 0x00;
    buf[6] = 0x01;
    assert_eq!(
        HandshakeMessagePayload::read_bytes(&buf[..0x10001 + 7]).unwrap_err(),
        InvalidMessage::CertificatePayloadTooLarge
    );
}

#[test]
fn can_decode_server_hello_from_api_devicecheck_apple_com() {
    let data = include_bytes!("../testdata/hello-api.devicecheck.apple.com.bin");
    let mut r = Reader::init(data);
    let hm = HandshakeMessagePayload::read(&mut r).unwrap();
    println!("msg: {hm:?}");
}

#[test]
fn wrapped_dn_encoding() {
    let subject = b"subject";
    let dn = DistinguishedName::in_sequence(&subject[..]);
    const DER_SEQUENCE_TAG: u8 = 0x30;
    let expected_prefix = vec![DER_SEQUENCE_TAG, subject.len() as u8];
    assert_eq!(dn.as_ref(), [expected_prefix, subject.to_vec()].concat());
}

fn sample_hello_retry_request() -> HelloRetryRequest {
    HelloRetryRequest {
        legacy_version: ProtocolVersion::TLSv1_2,
        session_id: SessionId::empty(),
        cipher_suite: CipherSuite::TLS_NULL_WITH_NULL_NULL,
        extensions: HelloRetryRequestExtensions {
            key_share: Some(NamedGroup::X25519),
            cookie: Some(PayloadU16::new(vec![0])),
            supported_versions: Some(ProtocolVersion::TLSv1_2),
            encrypted_client_hello: Some(Payload::new(vec![1, 2, 3])),
            order: None,
        },
    }
}

fn sample_client_hello_payload() -> ClientHelloPayload {
    ClientHelloPayload {
        client_version: ProtocolVersion::TLSv1_2,
        random: Random::from([0; 32]),
        session_id: SessionId::empty(),
        cipher_suites: vec![CipherSuite::TLS_NULL_WITH_NULL_NULL],
        compression_methods: vec![Compression::Null],
        extensions: Box::new(ClientExtensions {
            server_name: Some(ServerNamePayload::from(
                &DnsName::try_from("hello").unwrap(),
            )),
            cookie: Some(PayloadU16::new(vec![1, 2, 3])),
            signature_schemes: Some(vec![SignatureScheme::ECDSA_NISTP256_SHA256]),
            session_ticket: Some(ClientSessionTicket::Request),
            ec_point_formats: Some(SupportedEcPointFormats::default()),
            named_groups: Some(vec![NamedGroup::X25519]),
            protocols: Some(vec![ProtocolName::from(vec![0])]),
            supported_versions: Some(SupportedProtocolVersions {
                tls13: true,
                ..Default::default()
            }),
            key_shares: Some(vec![KeyShareEntry::new(NamedGroup::X25519, &[1, 2, 3][..])]),
            preshared_key_modes: Some(PskKeyExchangeModes {
                psk_dhe: true,
                psk: false,
            }),
            preshared_key_offer: Some(PresharedKeyOffer {
                identities: vec![
                    PresharedKeyIdentity::new(vec![3, 4, 5], 123456),
                    PresharedKeyIdentity::new(vec![6, 7, 8], 7891011),
                ],
                binders: vec![
                    PresharedKeyBinder::from(vec![1, 2, 3]),
                    PresharedKeyBinder::from(vec![3, 4, 5]),
                ],
            }),
            extended_master_secret_request: Some(()),
            certificate_status_request: Some(CertificateStatusRequest::build_ocsp()),
            server_certificate_types: Some(vec![CertificateType::RawPublicKey]),
            client_certificate_types: Some(vec![CertificateType::RawPublicKey]),
            transport_parameters: Some(Payload::new(vec![1, 2, 3])),
            early_data_request: Some(()),
            certificate_compression_algorithms: Some(vec![CertificateCompressionAlgorithm::Brotli]),
            encrypted_client_hello: Some(EncryptedClientHello::Inner),
            encrypted_client_hello_outer: Some(vec![ExtensionType::SCT]),
            ..Default::default()
        }),
    }
}

fn sample_server_hello_payload() -> ServerHelloPayload {
    ServerHelloPayload {
        legacy_version: ProtocolVersion::TLSv1_2,
        random: Random::from([0; 32]),
        session_id: SessionId::empty(),
        cipher_suite: CipherSuite::TLS_NULL_WITH_NULL_NULL,
        compression_method: Compression::Null,
        extensions: Box::new(ServerExtensions {
            ec_point_formats: Some(SupportedEcPointFormats::default()),
            server_name_ack: Some(()),
            session_ticket_ack: Some(()),
            renegotiation_info: Some(PayloadU8::new(vec![0])),
            selected_protocol: Some(SingleProtocolName::new(ProtocolName::from(vec![0]))),
            key_share: Some(KeyShareEntry::new(NamedGroup::X25519, &[1, 2, 3][..])),
            preshared_key: Some(3),
            early_data_ack: Some(()),
            encrypted_client_hello_ack: Some(ServerEncryptedClientHello {
                retry_configs: vec![],
            }),
            extended_master_secret_ack: Some(()),
            certificate_status_request_ack: Some(()),
            selected_version: Some(ProtocolVersion::TLSv1_2),
            transport_parameters: Some(Payload::new(vec![1, 2, 3])),
            transport_parameters_draft: None,
            client_certificate_type: Some(CertificateType::RawPublicKey),
            server_certificate_type: Some(CertificateType::RawPublicKey),
            unknown_extensions: Default::default(),
        }),
    }
}

fn all_tls12_handshake_payloads() -> Vec<HandshakeMessagePayload<'static>> {
    vec![
        HandshakeMessagePayload(HandshakePayload::HelloRequest),
        HandshakeMessagePayload(HandshakePayload::ClientHello(sample_client_hello_payload())),
        HandshakeMessagePayload(HandshakePayload::ServerHello(sample_server_hello_payload())),
        HandshakeMessagePayload(HandshakePayload::HelloRetryRequest(
            sample_hello_retry_request(),
        )),
        HandshakeMessagePayload(HandshakePayload::Certificate(CertificateChain(vec![
            CertificateDer::from(vec![1, 2, 3]),
        ]))),
        HandshakeMessagePayload(HandshakePayload::ServerKeyExchange(
            sample_ecdhe_server_key_exchange_payload(),
        )),
        HandshakeMessagePayload(HandshakePayload::ServerKeyExchange(
            sample_dhe_server_key_exchange_payload(),
        )),
        HandshakeMessagePayload(HandshakePayload::ServerKeyExchange(
            sample_unknown_server_key_exchange_payload(),
        )),
        HandshakeMessagePayload(HandshakePayload::CertificateRequest(
            sample_certificate_request_payload(),
        )),
        HandshakeMessagePayload(HandshakePayload::ServerHelloDone),
        HandshakeMessagePayload(HandshakePayload::ClientKeyExchange(Payload::Borrowed(&[
            1, 2, 3,
        ]))),
        HandshakeMessagePayload(HandshakePayload::NewSessionTicket(
            sample_new_session_ticket_payload(),
        )),
        HandshakeMessagePayload(HandshakePayload::EncryptedExtensions(
            sample_encrypted_extensions(),
        )),
        HandshakeMessagePayload(HandshakePayload::KeyUpdate(
            KeyUpdateRequest::UpdateRequested,
        )),
        HandshakeMessagePayload(HandshakePayload::KeyUpdate(
            KeyUpdateRequest::UpdateNotRequested,
        )),
        HandshakeMessagePayload(HandshakePayload::Finished(Payload::Borrowed(&[1, 2, 3]))),
        HandshakeMessagePayload(HandshakePayload::CertificateStatus(
            sample_certificate_status(),
        )),
        HandshakeMessagePayload(HandshakePayload::Unknown((
            HandshakeType::Unknown(99),
            Payload::Borrowed(&[1, 2, 3]),
        ))),
    ]
}

fn all_tls13_handshake_payloads() -> Vec<HandshakeMessagePayload<'static>> {
    vec![
        HandshakeMessagePayload(HandshakePayload::HelloRequest),
        HandshakeMessagePayload(HandshakePayload::ClientHello(sample_client_hello_payload())),
        HandshakeMessagePayload(HandshakePayload::ServerHello(sample_server_hello_payload())),
        HandshakeMessagePayload(HandshakePayload::HelloRetryRequest(
            sample_hello_retry_request(),
        )),
        HandshakeMessagePayload(HandshakePayload::CertificateTls13(
            sample_certificate_payload_tls13(),
        )),
        HandshakeMessagePayload(HandshakePayload::CompressedCertificate(
            sample_compressed_certificate(),
        )),
        HandshakeMessagePayload(HandshakePayload::ServerKeyExchange(
            sample_ecdhe_server_key_exchange_payload(),
        )),
        HandshakeMessagePayload(HandshakePayload::ServerKeyExchange(
            sample_dhe_server_key_exchange_payload(),
        )),
        HandshakeMessagePayload(HandshakePayload::ServerKeyExchange(
            sample_unknown_server_key_exchange_payload(),
        )),
        HandshakeMessagePayload(HandshakePayload::CertificateRequestTls13(
            sample_certificate_request_payload_tls13(),
        )),
        HandshakeMessagePayload(HandshakePayload::CertificateVerify(
            DigitallySignedStruct::new(SignatureScheme::ECDSA_NISTP256_SHA256, vec![1, 2, 3]),
        )),
        HandshakeMessagePayload(HandshakePayload::ServerHelloDone),
        HandshakeMessagePayload(HandshakePayload::ClientKeyExchange(Payload::Borrowed(&[
            1, 2, 3,
        ]))),
        HandshakeMessagePayload(HandshakePayload::NewSessionTicketTls13(
            sample_new_session_ticket_payload_tls13(),
        )),
        HandshakeMessagePayload(HandshakePayload::EncryptedExtensions(
            sample_encrypted_extensions(),
        )),
        HandshakeMessagePayload(HandshakePayload::KeyUpdate(
            KeyUpdateRequest::UpdateRequested,
        )),
        HandshakeMessagePayload(HandshakePayload::KeyUpdate(
            KeyUpdateRequest::UpdateNotRequested,
        )),
        HandshakeMessagePayload(HandshakePayload::Finished(Payload::Borrowed(&[1, 2, 3]))),
        HandshakeMessagePayload(HandshakePayload::CertificateStatus(
            sample_certificate_status(),
        )),
        HandshakeMessagePayload(HandshakePayload::Unknown((
            HandshakeType::Unknown(99),
            Payload::Borrowed(&[1, 2, 3]),
        ))),
    ]
}

fn sample_certificate_payload_tls13() -> CertificatePayloadTls13<'static> {
    CertificatePayloadTls13 {
        context: PayloadU8::new(vec![1, 2, 3]),
        entries: vec![CertificateEntry {
            cert: CertificateDer::from(vec![3, 4, 5]),
            extensions: CertificateExtensions {
                status: Some(CertificateStatus {
                    ocsp_response: PayloadU24(Payload::new(vec![1, 2, 3])),
                }),
            },
        }],
    }
}

fn sample_compressed_certificate() -> CompressedCertificatePayload<'static> {
    CompressedCertificatePayload {
        alg: CertificateCompressionAlgorithm::Brotli,
        uncompressed_len: 123,
        compressed: PayloadU24(Payload::new(vec![1, 2, 3])),
    }
}

fn sample_ecdhe_server_key_exchange_payload() -> ServerKeyExchangePayload {
    ServerKeyExchangePayload::Known(ServerKeyExchange {
        params: ServerKeyExchangeParams::Ecdh(ServerEcdhParams {
            curve_params: EcParameters {
                curve_type: ECCurveType::NamedCurve,
                named_group: NamedGroup::X25519,
            },
            public: PayloadU8::new(vec![1, 2, 3]),
        }),
        dss: DigitallySignedStruct::new(SignatureScheme::RSA_PSS_SHA256, vec![1, 2, 3]),
    })
}

fn sample_dhe_server_key_exchange_payload() -> ServerKeyExchangePayload {
    ServerKeyExchangePayload::Known(ServerKeyExchange {
        params: ServerKeyExchangeParams::Dh(ServerDhParams {
            dh_p: PayloadU16::new(vec![1, 2, 3]),
            dh_g: PayloadU16::new(vec![2]),
            dh_Ys: PayloadU16::new(vec![1, 2]),
        }),
        dss: DigitallySignedStruct::new(SignatureScheme::RSA_PSS_SHA256, vec![1, 2, 3]),
    })
}

fn sample_unknown_server_key_exchange_payload() -> ServerKeyExchangePayload {
    ServerKeyExchangePayload::Unknown(Payload::Borrowed(&[1, 2, 3]))
}

fn sample_certificate_request_payload() -> CertificateRequestPayload {
    CertificateRequestPayload {
        certtypes: vec![ClientCertificateType::RSASign],
        sigschemes: vec![SignatureScheme::ECDSA_NISTP256_SHA256],
        canames: vec![DistinguishedName::from(vec![1, 2, 3])],
    }
}

fn sample_certificate_request_payload_tls13() -> CertificateRequestPayloadTls13 {
    CertificateRequestPayloadTls13 {
        context: PayloadU8::new(vec![1, 2, 3]),
        extensions: CertificateRequestExtensions {
            signature_algorithms: Some(vec![SignatureScheme::ECDSA_NISTP256_SHA256]),
            authority_names: Some(vec![DistinguishedName::from(vec![1, 2, 3])]),
            certificate_compression_algorithms: Some(vec![CertificateCompressionAlgorithm::Zlib]),
        },
    }
}

fn sample_new_session_ticket_payload() -> NewSessionTicketPayload {
    NewSessionTicketPayload {
        lifetime_hint: 1234,
        ticket: Arc::new(PayloadU16::new(vec![1, 2, 3])),
    }
}

fn sample_new_session_ticket_payload_tls13() -> NewSessionTicketPayloadTls13 {
    NewSessionTicketPayloadTls13 {
        lifetime: 123,
        age_add: 1234,
        nonce: PayloadU8::new(vec![1, 2, 3]),
        ticket: Arc::new(PayloadU16::new(vec![4, 5, 6])),
        extensions: NewSessionTicketExtensions {
            max_early_data_size: Some(1234),
        },
    }
}

fn sample_encrypted_extensions() -> Box<ServerExtensions<'static>> {
    sample_server_hello_payload().extensions
}

fn sample_certificate_status() -> CertificateStatus<'static> {
    CertificateStatus {
        ocsp_response: PayloadU24(Payload::new(vec![1, 2, 3])),
    }
}
